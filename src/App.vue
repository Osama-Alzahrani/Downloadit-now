<template>
  <div class="h-screen bg-linear-to-br from-gray-900 via-slate-900 to-gray-900 flex flex-col">
    <!-- Header Bar -->
    <div class="bg-linear-to-r from-gray-900 to-slate-900 border-b border-teal-500/20 px-6 py-4 shadow-lg">
      <div class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-bold bg-linear-to-r from-teal-400 to-cyan-400 bg-clip-text text-transparent">YouTube Downloader</h1>
          <p class="text-xs text-gray-400 mt-1">Download videos with format selection</p>
        </div>
      </div>
    </div>

    <!-- Main Content -->
    <div class="flex-1 flex flex-col overflow-hidden p-4">
      <MainDownloader />
    </div>

    <!-- Footer -->
    <div class="bg-gray-900/50 border-t border-teal-500/20 px-6 py-3 text-xs text-gray-500 text-center">
      💾 Desktop • ⚡ yt-dlp
    </div>
  </div>
</template>

<script setup>
import MainDownloader from './components/MainDownloader.vue'
</script>
