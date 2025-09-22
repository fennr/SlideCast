<script setup lang="ts">
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { readFile } from "@tauri-apps/plugin-fs";
import SinglePagePreview from "./components/SinglePagePreview.vue";

type OverlayPosition = "top-left" | "top-right" | "bottom-left" | "bottom-right";
type ForegroundKind = "slides" | "video";
type QualityProfile = "draft" | "standard" | "high";
type OutputFormat = "mp4" | "mkv";

const step = ref(1);
const pdfPath = ref("");
const videoPath = ref("");
const outputPath = ref("");
const outputDir = ref<string>("");
const outputName = ref<string>("output");
const outputFormat = ref<OutputFormat>("mp4");
const overlayPosition = ref<OverlayPosition>("bottom-right");
const overlayPercent = ref(25); // 5..50
const foregroundKind = ref<ForegroundKind>("video");
const quality = ref<QualityProfile>("draft");
const pageCount = ref<number | null>(null);
const timings = ref<{ slide_index: number; time_seconds: number }[]>([]);
const ffmpegPath = ref<string | null>(null);

const pdfFile = ref<File | null>(null);

function recomputeUniformTimings() {
  if (!pageCount.value || !videoDuration.value) return false;
  const n = pageCount.value;
  const dur = Math.max(0.1, videoDuration.value);
  const step = dur / n;
  timings.value = Array.from({ length: n }, (_, i) => ({
    slide_index: i,
    time_seconds: +(i * step).toFixed(3),
  }));
  return true;
}

async function pickPdfViaDialog() {
  const path = await openDialog({ title: "Выберите PDF", filters: [{ name: "PDF", extensions: ["pdf"] }] });
  if (typeof path !== "string") return;
  pdfPath.value = path;
  // загрузим в память для предпросмотра
  const bytes = await readFile(path);
  const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes as any);
  pdfFile.value = new File([new Blob([u8])], "slides.pdf", { type: "application/pdf" });
  const count = await invoke<number>("get_pdf_page_count", { args: { pdf_path: pdfPath.value } });
  pageCount.value = count;
  if (!recomputeUniformTimings()) {
    timings.value = Array.from({ length: count }, (_, i) => ({ slide_index: i, time_seconds: i === 0 ? 0 : i * 5 }));
  }
}

async function pickVideoViaDialog() {
  const path = await openDialog({ title: "Выберите видео", filters: [{ name: "Видео", extensions: ["mp4","mov","mkv","webm","avi"] }] });
  if (typeof path !== "string") return;
  videoPath.value = path;
  try {
    videoDuration.value = await invoke<number>("probe_video_duration", { args: { video_path: videoPath.value } });
  } catch {
    videoDuration.value = null;
  }
  // если уже знаем число страниц — пересчитаем тайминги равномерно
  recomputeUniformTimings();
}

async function pickOutputDirViaDialog() {
  const path = await openDialog({ title: "Директория вывода", directory: true });
  if (typeof path !== "string") return;
  outputDir.value = path;
}

async function loadFfmpegPath() {
  const available = await invoke<boolean>("is_ffmpeg_available");
  if (available) {
    ffmpegPath.value = null;
  } else {
    ffmpegPath.value = (await invoke<string | null>("get_ffmpeg_path_configured")) ?? null;
  }
}

async function saveFfmpegPath() {
  await invoke("set_ffmpeg_path_configured", { path: ffmpegPath.value });
}
async function pickFfmpegViaDialog() {
  const path = await openDialog({ title: "Укажите путь к ffmpeg", filters: [{ name: "ffmpeg", extensions: ["exe",""] }], multiple: false });
  if (typeof path !== "string") return;
  ffmpegPath.value = path;
  await saveFfmpegPath();
}

const overlayRel = computed(() => Math.max(5, Math.min(50, overlayPercent.value)) / 100);
function gotoPreview() {
  if (!pdfFile.value || !videoPath.value || !pageCount.value) {
    statusMsg.value = "Выберите PDF и видео, дождитесь подсчёта страниц";
    return;
  }
  step.value = 2;
}
function backToSelect() {
  step.value = 1;
}

const isBusy = ref(false);
const statusMsg = ref("");
const progressUnits = ref(0);
const videoDuration = ref<number | null>(null);
const totalUnits = computed(() => (pageCount.value ?? 0) + 2);
const progressPercent = computed(() => {
  const total = totalUnits.value;
  if (!total) return 0;
  return Math.min(100, Math.round((progressUnits.value / total) * 100));
});

async function renderSlidesAndCompose() {
  if (!pdfFile.value || !pdfPath.value || !videoPath.value) return;
  isBusy.value = true;
  step.value = 3;
  try {
    progressUnits.value = 0;
    try {
      videoDuration.value = await invoke<number>("probe_video_duration", { args: { video_path: videoPath.value } });
    } catch {}
    statusMsg.value = "Создание временной папки";
    const framesDir = await invoke<string>("create_temp_dir", { prefix: "slides" });

    statusMsg.value = "Загрузка PDF";
    const pdfjs = await import("pdfjs-dist");
    // worker глобально настроен в PdfPreview
    const data = await pdfFile.value.arrayBuffer();
    const loadingTask = pdfjs.getDocument({ data, disableFontFace: true });
    const pdf = await loadingTask.promise;

    statusMsg.value = "Рендер страниц";
    for (let pageNum = 1; pageNum <= pdf.numPages; pageNum++) {
      const page = await pdf.getPage(pageNum);
      const viewport = page.getViewport({ scale: 2.0 });
      const canvas = document.createElement("canvas");
      const ctx = canvas.getContext("2d");
      if (!ctx) continue;
      canvas.width = Math.floor(viewport.width);
      canvas.height = Math.floor(viewport.height);
      await page.render({ canvasContext: ctx, viewport }).promise;
      const dataUrl = canvas.toDataURL("image/png");
      const base64 = dataUrl.replace(/^data:image\/png;base64,/, "");
      await invoke("save_slide_image", {
        args: {
          output_dir: framesDir,
          index: pageNum - 1,
          png_base64: base64,
        },
      });
      progressUnits.value = Math.min(totalUnits.value, progressUnits.value + 1);
    }

    statusMsg.value = "Формирование длительностей";
    const sorted = [...timings.value].sort((a, b) => a.slide_index - b.slide_index);
    const durations: number[] = [];
    for (let i = 0; i < sorted.length; i++) {
      const cur = sorted[i].time_seconds;
      const next = i + 1 < sorted.length ? sorted[i + 1].time_seconds : cur + 5.0;
      durations.push(Math.max(0.1, next - cur));
    }
    if (videoDuration.value) {
      const sum = durations.reduce((a, b) => a + b, 0);
      if (sum > videoDuration.value) {
        durations[durations.length - 1] = Math.max(0.1, durations[durations.length - 1] - (sum - videoDuration.value));
      }
    }

    statusMsg.value = "Сборка видео слайдов";
    const slidesVideo = `${framesDir}/slides.mp4`;
    await invoke("build_slides_video_with_durations", {
      args: {
        frames_dir: framesDir,
        durations,
        output_path: slidesVideo,
      },
    });
    progressUnits.value = Math.min(totalUnits.value, progressUnits.value + 1);

    statusMsg.value = "Композиция финального видео";
    const sep = outputDir.value.includes("\\") ? "\\" : "/";
    const finalOut = outputDir.value
      ? `${outputDir.value}${sep}${outputName.value}.${outputFormat.value}`
      : (outputPath.value || `output.${outputFormat.value}`);

    const request = {
      pdf_path: slidesVideo,
      video_path: videoPath.value,
      output_path: finalOut,
      overlay_position: overlayPosition.value,
      overlay_relative_width: overlayRel.value,
      foreground_kind: foregroundKind.value,
      quality: quality.value,
      fps: 30,
      output_width: 1920,
      output_height: 1080,
      expected_duration_sec: videoDuration.value ?? null,
      timings: timings.value,
    };
    await invoke("compose_video", { args: { request } });
    statusMsg.value = "Готово";
    progressUnits.value = totalUnits.value;
  } catch (e: any) {
    statusMsg.value = `Ошибка: ${e?.toString?.() ?? e}`;
  } finally {
    isBusy.value = false;
  }
}

loadFfmpegPath();
</script>

<template>
  <main class="container">
    <h2>SlideCast</h2>

    <a-card v-if="step===1" :bordered="false">
      <a-space direction="vertical" size="middle" style="width:100%">
        <a-space wrap>
          <a-button type="primary" @click="pickPdfViaDialog">Выбрать PDF…</a-button>
          <a-tag v-if="pageCount!==null">Страниц: {{ pageCount }}</a-tag>
        </a-space>
        <a-space wrap>
          <a-button type="primary" @click="pickVideoViaDialog">Выбрать видео…</a-button>
          <a-typography-text v-if="videoPath" type="secondary">{{ videoPath }}</a-typography-text>
        </a-space>
        <a-divider plain>Выходной файл</a-divider>
        <a-space wrap>
          <a-input v-model:value="outputDir" style="width:360px" placeholder="Директория" />
          <a-button @click="pickOutputDirViaDialog">Выбрать директорию…</a-button>
        </a-space>
        <a-space wrap>
          <a-input v-model:value="outputName" style="width:240px" placeholder="Имя файла без расширения" />
          <a-radio-group v-model:value="outputFormat">
            <a-radio-button value="mp4">MP4 (H.264)</a-radio-button>
            <a-radio-button value="mkv">MKV</a-radio-button>
          </a-radio-group>
        </a-space>
        <div />
      </a-space>
    </a-card>

    <a-card v-if="step===1 && pageCount" :bordered="false">
      <a-space wrap>
        <a-segmented v-model:value="overlayPosition" :options="[
          { label: 'Сверху слева', value: 'top-left' },
          { label: 'Сверху справа', value: 'top-right' },
          { label: 'Снизу слева', value: 'bottom-left' },
          { label: 'Снизу справа', value: 'bottom-right' },
        ]" />
        <a-space>
          <span>Размер: {{ overlayPercent }}%</span>
          <a-slider :min="5" :max="50" :step="1" v-model:value="overlayPercent" style="width:200px" />
        </a-space>
        <a-space align="center" size="small">
          <a-typography-text>Уменьшить:</a-typography-text>
          <a-radio-group v-model:value="foregroundKind">
          <a-radio-button value="video">Видео</a-radio-button>
          <a-radio-button value="slides">Слайды</a-radio-button>
          </a-radio-group>
        </a-space>
        <a-space align="center" size="small">
          <a-typography-text>Качество:</a-typography-text>
          <a-segmented v-model:value="quality" :options="[
            { label: 'Draft', value: 'draft' },
            { label: 'Standard', value: 'standard' },
            { label: 'High', value: 'high' },
          ]" />
        </a-space>
      </a-space>
    </a-card>

    <a-card v-if="step===2 && pageCount && pdfFile" :bordered="false">
      <h3>Тайминги</h3>
      <a-list :data-source="timings">
        <template #renderItem="{ item }">
          <a-list-item>
            <div class="timing-row">
              <div class="left">
                <a-space>
                  <a-tag color="blue">{{ item.slide_index + 1 }}</a-tag>
                  <a-input-number v-model:value="item.time_seconds" :step="0.1" />
                  <span>сек</span>
                </a-space>
              </div>
              <div class="thumb">
                <SinglePagePreview :file="pdfFile" :index="item.slide_index" :scale="0.3" />
              </div>
            </div>
          </a-list-item>
        </template>
      </a-list>
    </a-card>

    <a-card v-if="step===1" :bordered="false">
      <a-space direction="vertical" size="middle" style="width:100%">
        <a-space>
          <a-input v-model:value="ffmpegPath" style="width:360px" placeholder="например C:\\ffmpeg\\bin\\ffmpeg.exe" />
          <a-button @click="pickFfmpegViaDialog">Выбрать…</a-button>
          <a-button @click="saveFfmpegPath">Сохранить</a-button>
        </a-space>
        <a-typography-text type="secondary">Приложение попробует найти ffmpeg в PATH автоматически.</a-typography-text>
        <a-space>
          <a-button type="primary" :disabled="!pdfFile || !videoPath || !pageCount" @click="gotoPreview">Далее</a-button>
        </a-space>
      </a-space>
    </a-card>

    <a-space v-if="step===2" style="display:flex; gap:8px; align-items:center;">
      <a-button @click="backToSelect">Назад</a-button>
      <a-button type="primary" :loading="isBusy" @click="renderSlidesAndCompose">Сгенерировать</a-button>
    </a-space>

    <a-card v-if="step===3" :bordered="false">
      <a-progress :percent="progressPercent" status="active" />
      <a-typography-text>{{ statusMsg }}</a-typography-text>
    </a-card>
  </main>
  
</template>

<style scoped>
.container { padding: 16px; display: flex; flex-direction: column; gap: 16px; background: var(--bg); color: var(--fg); min-height: 100vh; }
.row { display: flex; gap: 16px; flex-wrap: wrap; }
.col { display: flex; flex-direction: column; gap: 8px; }

.grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
.col-timings { display: flex; flex-direction: column; gap: 8px; }
.col-preview { display: flex; flex-direction: column; gap: 8px; }

.timing { display: grid; grid-template-columns: auto 120px auto; align-items: center; gap: 8px; padding: 8px; border-radius: 8px; background: var(--panel); border: 1px solid var(--border); }

.timing-row { display: grid; grid-template-columns: 1fr auto; align-items: center; gap: 12px; }
.timing-row .thumb { width: 120px; }

input[type="number"], input[type="text"], select { padding: 8px 10px; border: 1px solid var(--border); border-radius: 8px; background: var(--input); color: var(--fg); }
input[type="range"] { width: 220px; }

button { padding: 8px 14px; border-radius: 8px; border: 1px solid var(--border); background: var(--btn); color: var(--fg); cursor: pointer; }
button:hover { filter: brightness(1.05); }
button:disabled { opacity: 0.6; cursor: default; }

.progress { width: 100%; height: 10px; border-radius: 8px; background: var(--panel); border: 1px solid var(--border); overflow: hidden; }
.bar { height: 100%; background: linear-gradient(90deg, #4f8cff, #22c1c3); transition: width 0.3s ease; }

@media (prefers-color-scheme: dark) {
  :root {
    --bg: #0f1115; --fg: #e6e6e6; --panel: #151922; --border: #2a2f3a; --btn: #1b2330; --input: #11161f;
  }
}
@media (prefers-color-scheme: light) {
  :root {
    --bg: #f5f7fb; --fg: #1e1e1e; --panel: #ffffff; --border: #dfe3eb; --btn: #f1f4f9; --input: #ffffff;
  }
}
</style>