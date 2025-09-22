<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import * as pdfjs from "pdfjs-dist";
// @ts-ignore - vite url import for worker
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl as string;

const props = defineProps<{
  file: File | null;
}>();

const container = ref<HTMLDivElement | null>(null);

async function renderPdf(file: File) {
  if (!container.value) return;
  container.value.innerHTML = "";
  const data = await file.arrayBuffer();
  const loadingTask = pdfjs.getDocument({ data, disableFontFace: true });
  const pdf = await loadingTask.promise;
  for (let pageNum = 1; pageNum <= pdf.numPages; pageNum++) {
    const page = await pdf.getPage(pageNum);
    const viewport = page.getViewport({ scale: 0.5 });
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");
    if (!ctx) continue;
    canvas.width = viewport.width;
    canvas.height = viewport.height;
    await page.render({ canvasContext: ctx, viewport }).promise;
    const wrapper = document.createElement("div");
    wrapper.className = "thumb";
    wrapper.appendChild(canvas);
    const label = document.createElement("div");
    label.className = "label";
    label.textContent = `Стр. ${pageNum}`;
    wrapper.appendChild(label);
    container.value.appendChild(wrapper);
  }
}

watch(
  () => props.file,
  (f) => {
    if (f) renderPdf(f);
  },
  { immediate: true }
);

onMounted(() => {
  if (props.file) renderPdf(props.file);
});
</script>

<template>
  <div ref="container" class="pdf-thumbs" />
  
</template>

<style scoped>
.pdf-thumbs { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 8px; max-height: 70vh; overflow: auto; }
.thumb { display: flex; flex-direction: column; align-items: center; gap: 4px; border: 1px solid #ddd; padding: 6px; background: #fff; }
.label { font-size: 12px; color: #555; }
canvas { width: 100%; height: auto; }
</style>

