<template>
  <div class="h-screen bg-linear-to-br from-gray-900 via-slate-900 to-gray-900 flex flex-col">

    <!-- Setup Overlay -->
    <div v-if="setup.visible" class="absolute inset-0 z-50 flex flex-col items-center justify-center bg-gray-950/95 backdrop-blur">
      <img :src="lockupSvg" alt="Downloadit" class="h-12 w-auto mb-8" />

      <div v-if="setup.error" class="text-center px-8">
        <p class="text-red-400 font-semibold mb-2">Setup failed</p>
        <p class="text-gray-400 text-sm mb-6">{{ setup.error }}</p>
        <button @click="runSetup" class="px-5 py-2 bg-teal-500 hover:bg-teal-600 text-white text-sm font-semibold rounded-lg transition-colors">
          Retry
        </button>
      </div>

      <div v-else class="w-80 text-center">
        <p class="text-gray-300 text-sm font-medium mb-6">Setting up required tools…</p>

        <!-- yt-dlp row -->
        <div class="mb-5">
          <div class="flex justify-between text-xs text-gray-400 mb-1.5">
            <span>yt-dlp</span>
            <span>{{ setup.ytdlpDone ? '✓ Ready' : setup.tool === 'yt-dlp' ? `${setup.progress.toFixed(0)}%` : setup.ytdlpDone === false && setup.tool !== 'yt-dlp' ? 'Waiting…' : '' }}</span>
          </div>
          <div class="h-1.5 bg-gray-800 rounded-full overflow-hidden">
            <div class="h-full bg-teal-500 rounded-full transition-all duration-300"
              :style="{ width: setup.ytdlpDone ? '100%' : setup.tool === 'yt-dlp' ? setup.progress + '%' : '0%' }" />
          </div>
        </div>

        <!-- ffmpeg row -->
        <div class="mb-6">
          <div class="flex justify-between text-xs text-gray-400 mb-1.5">
            <span>ffmpeg</span>
            <span>{{ setup.ffmpegDone ? '✓ Ready' : setup.tool === 'ffmpeg' ? `${setup.progress.toFixed(0)}%` : '' }}</span>
          </div>
          <div class="h-1.5 bg-gray-800 rounded-full overflow-hidden">
            <div class="h-full bg-cyan-500 rounded-full transition-all duration-300"
              :style="{ width: setup.ffmpegDone ? '100%' : setup.tool === 'ffmpeg' ? setup.progress + '%' : '0%' }" />
          </div>
        </div>

        <p class="text-gray-500 text-xs">{{ setup.status }}</p>
      </div>
    </div>

    <!-- Header Bar -->
    <div class="bg-linear-to-r from-gray-900 to-slate-900 border-b border-teal-500/20 px-6 py-4 shadow-lg">
      <div class="flex items-center justify-between">
        <img :src="lockupSvg" alt="Downloadit" class="h-9 w-auto" />
        <div v-if="update.available && !update.installing" class="flex items-center gap-3">
          <span class="text-xs text-gray-400">v{{ update.version }} available</span>
          <button @click="installUpdate"
            class="px-3 py-1.5 bg-teal-500 hover:bg-teal-600 text-white text-xs font-semibold rounded-lg transition-colors">
            ↑ Update
          </button>
        </div>
        <div v-if="update.installing" class="flex items-center gap-3">
          <div class="flex flex-col items-end gap-1">
            <div class="flex items-center justify-between w-48 text-xs text-gray-400">
              <span>{{ update.status }}</span>
              <span class="font-semibold text-teal-400">{{ update.progress }}%</span>
            </div>
            <div class="w-48 h-1.5 bg-gray-700 rounded-full overflow-hidden">
              <div class="h-full bg-teal-500 rounded-full transition-all duration-200"
                :style="{ width: update.progress + '%' }" />
            </div>
          </div>
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
import { ref, reactive, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import MainDownloader from './components/MainDownloader.vue'
import lockupSvg from './assets/downloadit-lockup.svg'

let _updateHandle = null  // kept outside reactive to avoid Proxy wrapping private fields

const update = reactive({
  available: false,
  version: '',
  installing: false,
  progress: 0,
  status: '',
})

const installUpdate = async () => {
  if (!_updateHandle) return
  update.installing = true
  update.progress = 0
  update.status = 'Starting…'
  let downloaded = 0
  let total = 0
  await _updateHandle.downloadAndInstall((event) => {
    if (event.event === 'Started') {
      total = event.data.contentLength ?? 0
      update.status = 'Downloading…'
    } else if (event.event === 'Progress') {
      downloaded += event.data.chunkLength
      update.progress = total > 0 ? Math.round(downloaded / total * 100) : 0
      update.status = 'Downloading…'
    } else if (event.event === 'Finished') {
      update.progress = 100
      update.status = 'Restarting…'
    }
  })
  await relaunch()
}

const checkForUpdates = async () => {
  try {
    const result = await check()
    if (result?.available) {
      update.available = true
      update.version = result.version
      _updateHandle = result
    }
  } catch {
    // silently ignore — updater not configured yet or no network
  }
}

const setup = reactive({
  visible: false,
  tool: '',
  progress: 0,
  status: '',
  ytdlpDone: false,
  ffmpegDone: false,
  error: null,
})

let unlistenSetup = null

const runSetup = async () => {
  setup.error = null
  setup.visible = true
  try {
    await invoke('download_dependencies')
    setup.visible = false
  } catch (err) {
    setup.error = String(err)
  }
}

onMounted(async () => {
  checkForUpdates()

  unlistenSetup = await listen('setup-progress', (event) => {
    const { tool, progress, status } = event.payload
    setup.tool = tool
    setup.progress = progress
    setup.status = status
    if (tool === 'yt-dlp' && progress >= 100) setup.ytdlpDone = true
    if (tool === 'ffmpeg' && progress >= 99) setup.ffmpegDone = true
    if (tool === 'done') setup.visible = false
  })

  const deps = await invoke('check_dependencies')
  if (!deps.ytdlp || !deps.ffmpeg) {
    runSetup()
  }
})

onUnmounted(() => {
  if (unlistenSetup) unlistenSetup()
})
</script>
