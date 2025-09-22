<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import * as pdfjs from "pdfjs-dist";
// @ts-ignore
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
pdfjs.GlobalWorkerOptions.workerSrc = workerUrl as string;

const props = defineProps<{ file: File | null; index: number; scale?: number }>();
const canvasRef = ref<HTMLCanvasElement | null>(null);

async function renderPage() {
  if (!props.file || !canvasRef.value) return;
  const data = await props.file.arrayBuffer();
  const doc = await pdfjs.getDocument({ data, disableFontFace: true }).promise;
  const pageIndex = Math.max(0, Math.min(props.index, doc.numPages - 1));
  const page = await doc.getPage(pageIndex + 1);
  const viewport = page.getViewport({ scale: props.scale ?? 0.8 });
  const canvas = canvasRef.value;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  canvas.width = Math.floor(viewport.width);
  canvas.height = Math.floor(viewport.height);
  await page.render({ canvasContext: ctx, viewport }).promise;
}

watch(() => [props.file, props.index, props.scale], () => { renderPage(); });
onMounted(() => { renderPage(); });
</script>

<template>
  <canvas ref="canvasRef" style="max-width: 100%; height: auto;" />
  
</template>

<style scoped>
</style>

