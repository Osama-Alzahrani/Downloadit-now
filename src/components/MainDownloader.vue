<template>
  <!-- URL Input Bar -->
  <div class="bg-gray-800/30 border border-gray-700/50 rounded-lg p-3 mb-3">
    <div class="flex gap-2 mb-2">
      <input
        v-model="url"
        @keydown.enter="addToQueue"
        type="text"
        placeholder="Paste YouTube URL and press Enter..."
        class="flex-1 px-3 py-2 bg-gray-800 border border-gray-700 text-gray-100 rounded-lg text-sm placeholder:text-gray-500 focus:outline-none focus:border-teal-500 focus:shadow-lg focus:shadow-teal-500/20 transition-all"
      />
      <button
        @click="addToQueue"
        :disabled="!url.trim()"
        class="px-4 py-2 bg-teal-500 hover:bg-teal-600 disabled:opacity-40 disabled:cursor-not-allowed text-white font-bold rounded-lg text-sm transition-colors whitespace-nowrap"
      >
        + Add
      </button>
      <button
        v-if="queue.some(i => i.status === 'completed' || i.status === 'error')"
        @click="clearCompleted"
        class="px-3 py-2 bg-gray-700 hover:bg-gray-600 text-gray-300 text-sm rounded-lg transition-colors whitespace-nowrap"
        title="Clear completed and errored items"
      >
        🗑 Clear
      </button>
    </div>
    <div class="flex gap-2">
      <div class="flex-1 px-3 py-1.5 bg-gray-800 border border-gray-700 text-gray-400 rounded-lg text-xs flex items-center truncate">
        {{ downloadPath || '📁 Downloads folder (default)' }}
      </div>
      <button @click="pickDownloadPath" class="px-3 py-1.5 bg-gray-600 hover:bg-gray-700 text-white font-semibold rounded-lg text-xs transition-colors whitespace-nowrap">
        📂 Browse
      </button>
      <!-- Concurrent limit -->
      <div class="flex items-center gap-1.5 px-2 py-1 bg-gray-800 border border-gray-700 rounded-lg">
        <span class="text-xs text-gray-400 whitespace-nowrap">Concurrent:</span>
        <button @click="setConcurrent(concurrentLimit - 1)" :disabled="concurrentLimit <= 1" class="w-5 h-5 flex items-center justify-center text-gray-300 hover:text-white disabled:opacity-30 font-bold text-sm">−</button>
        <span class="text-xs font-bold text-teal-400 w-3 text-center">{{ concurrentLimit }}</span>
        <button @click="setConcurrent(concurrentLimit + 1)" :disabled="concurrentLimit >= 5" class="w-5 h-5 flex items-center justify-center text-gray-300 hover:text-white disabled:opacity-30 font-bold text-sm">+</button>
      </div>
    </div>
  </div>

  <!-- Queue -->
  <div class="flex-1 overflow-y-auto space-y-2 min-h-0">
    <!-- Empty state -->
    <div v-if="queue.length === 0" class="flex flex-col items-center justify-center py-16 text-gray-600">
      <img src="../assets/downloadit-glyph.svg" alt="" class="mb-3 opacity-40" />
      <p class="text-sm font-medium mb-1">No downloads queued</p>
      <p class="text-xs">Paste a YouTube URL above and press Enter</p>
    </div>

    <!-- Queue items -->
    <div
      v-for="item in queue"
      :key="item.id"
      class="rounded-lg overflow-hidden border transition-colors"
      :class="{
        'bg-gray-800/60 border-teal-500/40': item.status === 'downloading',
        'bg-gray-800/60 border-yellow-500/30': item.status === 'paused',
        'bg-gray-800/40 border-emerald-500/20': item.status === 'completed',
        'bg-gray-800/40 border-red-500/20': item.status === 'error',
        'bg-gray-800/40 border-gray-700/40': item.status === 'ready' || item.status === 'queued' || item.status === 'fetching',
      }"
    >
      <!-- Fetching skeleton -->
      <div v-if="item.status === 'fetching'" class="flex items-center gap-3 p-3">
        <div class="w-20 h-12 bg-gray-700/60 rounded animate-pulse shrink-0"></div>
        <div class="flex-1 space-y-2">
          <div class="h-3 bg-gray-700/60 rounded animate-pulse w-3/4"></div>
          <div class="h-2 bg-gray-700/60 rounded animate-pulse w-1/2"></div>
        </div>
        <button @click="removeItem(item.id)" class="text-gray-600 hover:text-gray-400 text-xl leading-none shrink-0 transition-colors">×</button>
      </div>

      <!-- Ready / Active / Done -->
      <div v-else>
        <!-- Card header -->
        <div class="flex items-start gap-3 p-3 pb-2">
          <!-- Thumbnail + downloading indicator -->
          <div class="relative shrink-0 self-center">
            <div v-if="item.status === 'downloading'" class="absolute -left-2.5 top-1/2 -translate-y-1/2 text-teal-400 text-base font-bold">↓</div>
            <img
              v-if="item.thumbnail"
              :src="item.thumbnail"
              class="w-20 h-12 object-cover rounded"
              @error="item.thumbnail = null"
            />
            <div v-else class="w-20 h-12 bg-gray-700 rounded flex items-center justify-center text-2xl">🎬</div>
          </div>

          <!-- Content -->
          <div class="flex-1 min-w-0">
            <div class="flex items-start justify-between gap-1 mb-1.5">
              <p class="text-sm font-semibold text-gray-100 leading-tight line-clamp-2">{{ item.title }}</p>
              <button @click="removeItem(item.id)" class="shrink-0 text-gray-600 hover:text-gray-300 text-xl leading-none ml-1 transition-colors">×</button>
            </div>

            <!-- Metadata badges -->
            <div class="flex items-center gap-1.5 flex-wrap">
              <span v-if="item.ext" class="px-1.5 py-0.5 bg-blue-500/25 border border-blue-500/40 text-blue-300 text-xs rounded font-mono uppercase">{{ item.ext }}</span>
              <span v-if="item.width && item.height" class="px-1.5 py-0.5 bg-gray-700/80 text-gray-300 text-xs rounded font-mono">{{ item.width }}×{{ item.height }}</span>
              <span v-if="item.duration" class="px-1.5 py-0.5 bg-gray-700/80 text-gray-300 text-xs rounded font-mono">{{ formatDuration(item.duration) }}</span>
              <span v-if="item.filesize" class="px-1.5 py-0.5 bg-gray-700/80 text-gray-300 text-xs rounded font-mono">{{ formatFilesize(item.filesize) }}</span>
              <span v-if="item.selectedFormatLabel" class="px-1.5 py-0.5 bg-teal-500/20 border border-teal-500/40 text-teal-300 text-xs rounded">{{ item.selectedFormatLabel }}</span>
            </div>

            <!-- Status -->
            <div v-if="item.status === 'completed'" class="flex items-center gap-2 mt-1">
              <span class="text-xs text-emerald-400 font-medium">✓ Downloaded</span>
              <button
                v-if="item.filePath"
                @click.stop="openFile(item.filePath)"
                class="px-1.5 py-0.5 bg-emerald-500/15 hover:bg-emerald-500/30 border border-emerald-500/30 text-emerald-400 text-[10px] font-semibold rounded transition-colors"
                title="Open file"
              >▶ Open</button>
              <button
                v-if="item.filePath"
                @click.stop="openFolder(item.filePath)"
                class="px-1.5 py-0.5 bg-gray-700/60 hover:bg-gray-600/60 text-gray-400 text-[10px] font-semibold rounded transition-colors"
                title="Show in folder"
              >📂 Folder</button>
            </div>
            <p v-else-if="item.status === 'error'" class="text-xs text-red-400 mt-1 truncate" :title="item.error">❌ {{ item.error }}</p>
            <p v-else-if="item.status === 'queued'" class="text-xs text-gray-500 mt-1">Queued...</p>
            <p v-else-if="item.status === 'paused'" class="text-xs text-yellow-400 mt-1">⏸ Paused at {{ Math.round(item.progress) }}%</p>
            <p v-else-if="item.status === 'ready' && !item.selectedFormat" class="text-xs text-amber-400 mt-1">Select a format below to download</p>
          </div>

          <!-- Right buttons -->
          <div class="shrink-0 flex flex-col gap-1 self-start">
            <!-- Show/hide formats toggle (only when ready or has format selected) -->
            <button
              v-if="item.status === 'ready'"
              @click="toggleFormats(item)"
              :class="[
                'px-2.5 py-1 text-xs font-semibold rounded transition-colors whitespace-nowrap',
                item.showFormats
                  ? 'bg-teal-600 hover:bg-teal-700 text-white'
                  : 'bg-gray-700 hover:bg-gray-600 text-gray-300'
              ]"
            >
              {{ item.showFormats ? '✕ Formats' : '📊 Formats' }}
            </button>
            <!-- Start with best quality -->
            <button
              v-if="item.status === 'ready'"
              @click="startBest(item.id)"
              class="px-2.5 py-1 bg-teal-500/20 hover:bg-teal-500/30 border border-teal-500/40 text-teal-300 text-xs font-semibold rounded transition-colors whitespace-nowrap"
            >
              ↓ Best
            </button>
            <!-- Downloading: Pause + Cancel -->
            <template v-if="item.status === 'downloading'">
              <button @click="pauseItem(item.id)" class="px-2.5 py-1 bg-yellow-500/20 hover:bg-yellow-500/30 border border-yellow-500/40 text-yellow-300 text-xs font-semibold rounded transition-colors whitespace-nowrap">
                ⏸ Pause
              </button>
              <button @click="cancelItem(item.id)" class="px-2.5 py-1 bg-red-500/20 hover:bg-red-500/30 border border-red-500/40 text-red-300 text-xs font-semibold rounded transition-colors whitespace-nowrap">
                ✕ Cancel
              </button>
            </template>
            <!-- Paused: Resume + Cancel -->
            <template v-else-if="item.status === 'paused'">
              <button @click="resumeItem(item.id)" class="px-2.5 py-1 bg-teal-500/20 hover:bg-teal-500/30 border border-teal-500/40 text-teal-300 text-xs font-semibold rounded transition-colors whitespace-nowrap">
                ▶ Resume
              </button>
              <button @click="cancelItem(item.id)" class="px-2.5 py-1 bg-red-500/20 hover:bg-red-500/30 border border-red-500/40 text-red-300 text-xs font-semibold rounded transition-colors whitespace-nowrap">
                ✕ Cancel
              </button>
            </template>
            <!-- Queued: Skip -->
            <button
              v-else-if="item.status === 'queued'"
              @click="cancelItem(item.id)"
              class="px-2.5 py-1 bg-gray-700 hover:bg-gray-600 text-gray-300 text-xs font-semibold rounded transition-colors"
            >
              SKIP
            </button>
          </div>
        </div>

        <!-- Progress bar -->
        <div v-if="item.status === 'downloading' || item.status === 'paused'" class="px-3 pb-2.5">
          <div class="w-full bg-gray-700/60 rounded-full h-1.5 overflow-hidden mb-1">
            <div
              class="h-full rounded-full transition-all duration-300"
              :class="item.status === 'paused' ? 'bg-gray-500' : 'bg-linear-to-r from-teal-500 to-cyan-400'"
              :style="{ width: `${item.progress}%` }"
            ></div>
          </div>
          <div class="flex justify-between text-xs text-gray-500">
            <span class="truncate pr-2">{{ item.statusText }}</span>
            <span class="shrink-0">{{ Math.round(item.progress) }}%</span>
          </div>
        </div>

        <!-- Format table (inline, expandable) -->
        <div v-if="item.showFormats" class="border-t border-gray-700/50">
          <!-- Loading formats -->
          <div v-if="item.loadingFormats" class="p-3 text-center text-xs text-gray-500">
            <span class="spinner-sm inline-block mr-1"></span> Fetching formats...
          </div>

          <!-- Format table -->
          <div v-else-if="item.formatTable && item.formatTable.length > 0" class="p-2">
            <p class="text-xs text-gray-400 mb-1.5 px-1">Click a row to download with that format. Click a column header to sort:</p>
            <div class="overflow-x-auto">
              <table class="w-full text-xs">
                <thead>
                  <tr class="text-gray-400 border-b border-gray-700 select-none">
                    <th v-for="col in formatColumns" :key="col.key"
                      class="py-1.5 px-1.5 font-semibold text-left cursor-pointer hover:text-teal-300 transition-colors whitespace-nowrap"
                      :class="{ 'text-teal-400': sortKey === col.key }"
                      @click="setSort(col.key)"
                    >
                      {{ col.label }}
                      <span v-if="sortKey === col.key" class="ml-0.5">{{ sortDir === 'asc' ? '↑' : '↓' }}</span>
                    </th>
                    <th class="py-1 px-1.5"></th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="fmt in sortedRows(item)"
                    :key="fmt.id"
                    class="border-b border-gray-800 hover:bg-teal-500/10 cursor-pointer transition-colors group"
                    @click="downloadWithFormat(item.id, fmt.id, fmt)"
                  >
                    <td class="py-1.5 px-1.5 font-bold text-blue-400">{{ fmt.id }}</td>
                    <td class="py-1.5 px-1.5 font-semibold text-blue-300">{{ fmt.ext }}</td>
                    <td class="py-1.5 px-1.5 text-gray-300">{{ fmt.resolution }}</td>
                    <td class="py-1.5 px-1.5 text-gray-400">{{ fmt.fps }}</td>
                    <td class="py-1.5 px-1.5 text-emerald-400 font-mono">{{ fmt.filesize }}</td>
                    <td class="py-1.5 px-1.5 text-amber-400 font-mono">{{ fmt.tbr }}</td>
                    <td class="py-1.5 px-1.5 font-mono truncate max-w-20" :class="fmt.audioOnly ? 'text-gray-500 italic' : 'text-cyan-300'">{{ fmt.audioOnly ? '—' : fmt.vcodec }}</td>
                    <td class="py-1.5 px-1.5 font-mono truncate max-w-20">
                      <span v-if="fmt.videoOnly" class="text-gray-500 italic text-[10px]">none</span>
                      <span v-else-if="fmt.audioOnly" class="text-pink-300">{{ fmt.acodec }}</span>
                      <span v-else class="text-pink-300">{{ fmt.acodec }}</span>
                    </td>
                    <td class="py-1.5 px-1.5 whitespace-nowrap">
                      <span v-if="fmt.videoOnly" class="text-yellow-500/70 text-[10px] mr-1" title="Video only — audio will be merged automatically">+🔊</span>
                      <span class="opacity-0 group-hover:opacity-100 text-teal-400 font-bold transition-opacity">↓</span>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>

          <!-- Format error -->
          <div v-else class="p-3 text-xs text-red-400">
            {{ item.formatsError || 'No formats found.' }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'

const emit = defineEmits(['history-update'])

const url = ref('')
const downloadPath = ref(null)
const queue = ref([])
let idCounter = 0
const concurrentLimit = ref(Number(localStorage.getItem('concurrentLimit') || 2))

const setConcurrent = (n) => {
  concurrentLimit.value = Math.max(1, Math.min(5, n))
  localStorage.setItem('concurrentLimit', concurrentLimit.value)
  startNextIfIdle() // fill any newly opened slots
}

// Persist queue to localStorage on every change (skip transient UI-only fields)
watch(queue, (q) => {
  localStorage.setItem('downloadQueue', JSON.stringify(q.map(item => ({
    id: item.id,
    url: item.url,
    title: item.title,
    thumbnail: item.thumbnail,
    duration: item.duration,
    filesize: item.filesize,
    ext: item.ext,
    width: item.width,
    height: item.height,
    selectedFormat: item.selectedFormat,
    selectedFormatLabel: item.selectedFormatLabel,
    progress: item.progress,
    status: item.status,
    statusText: item.statusText,
    error: item.error,
    filePath: item.filePath,
  }))))
}, { deep: true })

// Sort state for the format table (shared — only one table open at a time)
const sortKey = ref('id')
const sortDir = ref('asc')

const formatColumns = [
  { key: 'id',         label: 'ID' },
  { key: 'ext',        label: 'Ext' },
  { key: 'resolution', label: 'Resolution' },
  { key: 'fps',        label: 'FPS' },
  { key: 'filesize',   label: 'Size' },
  { key: 'tbr',        label: 'Bitrate' },
  { key: 'vcodec',     label: 'Video' },
  { key: 'acodec',     label: 'Audio' },
]

const sortValue = (row, key) => {
  const v = row[key]
  if (key === 'id' || key === 'fps' || key === 'ch') return parseInt(v) || 0
  if (key === 'filesize') {
    const m = String(v).match(/[\d.]+/)
    const n = parseFloat(m?.[0] || 0)
    if (/GiB|GB/i.test(v)) return n * 1024
    if (/MiB|MB/i.test(v)) return n
    if (/KiB|KB/i.test(v)) return n / 1024
    return n
  }
  if (key === 'tbr') {
    const m = String(v).match(/[\d.]+/)
    return parseFloat(m?.[0] || 0)
  }
  if (key === 'resolution') {
    const m = String(v).match(/(\d+)x(\d+)/)
    return m ? parseInt(m[2]) : 0
  }
  return String(v || '').toLowerCase()
}

const setSort = (key) => {
  if (sortKey.value === key) {
    sortDir.value = sortDir.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortKey.value = key
    sortDir.value = 'asc'
  }
}

const sortedRows = (item) => {
  if (!item.formatTable) return []
  return [...item.formatTable].sort((a, b) => {
    const av = sortValue(a, sortKey.value)
    const bv = sortValue(b, sortKey.value)
    if (av < bv) return sortDir.value === 'asc' ? -1 : 1
    if (av > bv) return sortDir.value === 'asc' ? 1 : -1
    return 0
  })
}

// Format table live output buffer per item (keyed by item id)
const formatOutputBuffer = {}

const addToQueue = async () => {
  const trimmed = url.value.trim()
  if (!trimmed) return

  url.value = ''
  const id = String(++idCounter)

  queue.value.push({
    id,
    url: trimmed,
    title: trimmed,
    thumbnail: null,
    duration: null,
    filesize: null,
    ext: null,
    width: null,
    height: null,
    selectedFormat: null,
    selectedFormatLabel: null,
    progress: 0,
    status: 'fetching',
    statusText: '',
    error: null,
    filePath: null,
    showFormats: false,
    loadingFormats: false,
    formatTable: [],
    formatsError: null,
  })

  try {
    const info = await invoke('get_video_info', { url: trimmed })
    const item = queue.value.find(i => i.id === id)
    if (item) {
      item.title = info.title || trimmed
      item.thumbnail = info.thumbnail || null
      item.duration = info.duration || null
      item.filesize = info.filesize || null
      item.ext = info.ext || null
      item.width = info.width || null
      item.height = info.height || null
      item.status = 'ready'
    }
  } catch (err) {
    const item = queue.value.find(i => i.id === id)
    if (item) {
      item.status = 'error'
      item.error = String(err)
    }
  }
}

const toggleFormats = async (item) => {
  item.showFormats = !item.showFormats
  if (item.showFormats && item.formatTable.length === 0 && !item.loadingFormats) {
    await fetchFormats(item)
  }
}

const fetchFormats = async (item) => {
  item.loadingFormats = true
  item.formatsError = null
  formatOutputBuffer[item.id] = ''

  try {
    await invoke('get_video_formats', { url: item.url })
    // Formats parsed from format-output events via the buffer
    parseFormatTable(item)
  } catch (err) {
    item.formatsError = String(err)
  } finally {
    item.loadingFormats = false
  }
}

function parseFormatTable(item) {
  const lines = (formatOutputBuffer[item.id] || '').split('\n')
  const rows = []
  let inTable = false

  for (const line of lines) {
    const trimmed = line.trim()
    if (!trimmed) continue
    if (trimmed.includes('ID') && trimmed.includes('EXT')) { inTable = true; continue }
    if (trimmed.startsWith('---') || trimmed.startsWith('===')) continue
    if (trimmed.includes('WARNING') || trimmed.includes('[info]')) { inTable = false; continue }

    if (inTable && /^\d+\s+/.test(trimmed)) {
      const sections = line.split('|').map(s => s.trim())
      const firstPart = sections[0].split(/\s+/)
      const id = firstPart[0]
      const ext = firstPart[1] || '-'
      let resolution = '-', fps = '-', ch = '-'
      if (firstPart[2]) {
        if (firstPart[2].includes('x') || firstPart[2].includes('audio')) {
          resolution = firstPart[2]
          fps = /^\d+$/.test(firstPart[3]) ? firstPart[3] : '-'
          ch = /^\d+$/.test(firstPart[4]) ? firstPart[4] : '-'
        }
      }
      let filesize = '-', tbr = '-', proto = '-'
      if (sections[1]) {
        const parts = sections[1].split(/\s+/)
        filesize = parts[0] || '-'; tbr = parts[1] || '-'; proto = parts[2] || '-'
      }
      let vcodec = '-', acodec = '-'
      if (sections.length > 2) {
        const codecParts = sections[2].split(/\s+/)
        vcodec = codecParts[0] || '-'
        acodec = codecParts[codecParts.length - 1] || '-'
      }
      const videoOnly = /video.?only/i.test(trimmed)
      const audioOnly = /audio.?only/i.test(trimmed) || resolution === 'audio only'
      rows.push({ id, ext, resolution, fps, ch, filesize, tbr, proto, vcodec, acodec, videoOnly, audioOnly })
    }
  }
  item.formatTable = rows
}

const downloadWithFormat = (itemId, formatId, fmt) => {
  const item = queue.value.find(i => i.id === itemId)
  if (!item || item.status !== 'ready') return

  // Video-only DASH tracks have no audio stream — auto-merge with best available audio
  const effectiveFormat = fmt.videoOnly
    ? `${formatId}+bestaudio[ext=m4a]/bestaudio`
    : formatId

  const resLabel = fmt.resolution !== '-' ? fmt.resolution : 'audio'
  item.selectedFormat = effectiveFormat
  item.selectedFormatLabel = `${fmt.ext?.toUpperCase()} ${resLabel} (${formatId})${fmt.videoOnly ? ' +audio' : ''}`
  item.showFormats = false
  enqueueItem(itemId)
}

const startBest = (itemId) => {
  const item = queue.value.find(i => i.id === itemId)
  if (!item) return
  item.selectedFormat = null
  item.selectedFormatLabel = null
  enqueueItem(itemId)
}

const enqueueItem = (itemId) => {
  const item = queue.value.find(i => i.id === itemId)
  if (!item || item.status !== 'ready') return
  item.status = 'queued'
  item.showFormats = false
  startNextIfIdle()
}

const startNextIfIdle = () => {
  const active = queue.value.filter(i => i.status === 'downloading').length
  const slots = concurrentLimit.value - active
  if (slots <= 0) return
  queue.value
    .filter(i => i.status === 'queued')
    .slice(0, slots)
    .forEach(item => startDownload(item))
}

const startDownload = async (item) => {
  item.status = 'downloading'
  item.progress = 0
  item.statusText = ''
  item.error = null

  try {
    await invoke('download_video_stream', {
      params: {
        url: item.url,
        format: item.selectedFormat || null,
        download_path: downloadPath.value,
        download_id: item.id,
      }
    })
  } catch (err) {
    const i = queue.value.find(q => q.id === item.id)
    if (i && i.status === 'downloading') {
      i.status = 'error'
      i.error = String(err)
      startNextIfIdle()
    }
    // If status is 'paused', do nothing — pauseItem already handled it
  }
}

const pauseItem = async (id) => {
  const item = queue.value.find(i => i.id === id)
  if (!item || item.status !== 'downloading') return
  const savedProgress = item.progress
  const savedStatusText = item.statusText
  // Set paused BEFORE await so the download-error listener sees it and doesn't treat it as an error
  item.status = 'paused'
  try {
    await invoke('pause_download', { downloadId: id })
    item.progress = savedProgress
    item.statusText = savedStatusText
  } catch {
    item.status = 'downloading'
    return
  }
  // Fill the freed slot with next queued item
  startNextIfIdle()
}

const resumeItem = (id) => {
  const item = queue.value.find(i => i.id === id)
  if (!item || item.status !== 'paused') return
  item.status = 'queued'
  startNextIfIdle()
}

const cancelItem = async (id) => {
  const item = queue.value.find(i => i.id === id)
  if (!item) return
  if (item.status === 'downloading' || item.status === 'paused') {
    try { await invoke('cancel_download', { downloadId: id }) } catch {}
  }
  queue.value = queue.value.filter(i => i.id !== id)
  startNextIfIdle()
}

const skipItem = async (id) => {
  await cancelItem(id)
}

const removeItem = (id) => cancelItem(id)

const pickDownloadPath = async () => {
  try {
    const selected = await open({ directory: true, multiple: false })
    if (selected) {
      downloadPath.value = selected
      localStorage.setItem('downloadPath', selected)
    }
  } catch {}
}

const formatDuration = (seconds) => {
  if (!seconds) return ''
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = Math.floor(seconds % 60)
  const pad = n => String(n).padStart(2, '0')
  return h > 0 ? `${pad(h)}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`
}

const formatFilesize = (bytes) => {
  if (!bytes) return ''
  if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(2)}GB`
  if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(2)}MB`
  return `${(bytes / 1024).toFixed(1)}KB`
}

const saveToHistory = (item, filePath) => {
  try {
    const history = JSON.parse(localStorage.getItem('downloadHistory') || '[]')
    history.unshift({
      id: `${Date.now()}-${item.id}`,
      url: item.url,
      title: item.title,
      thumbnail: item.thumbnail,
      filePath: filePath || null,
      ext: item.ext,
      width: item.width,
      height: item.height,
      duration: item.duration,
      selectedFormatLabel: item.selectedFormatLabel,
      downloadedAt: new Date().toISOString(),
    })
    if (history.length > 200) history.splice(200)
    localStorage.setItem('downloadHistory', JSON.stringify(history))
    emit('history-update', history.length)
  } catch {}
}

const clearCompleted = () => {
  queue.value = queue.value.filter(i => i.status !== 'completed' && i.status !== 'error')
}

const openFile = async (filePath) => {
  if (!filePath) return
  try {
    await invoke('open_file', { path: filePath })
  } catch (err) {
    console.error('Failed to open file:', err)
  }
}

const openFolder = async (filePath) => {
  if (!filePath) return
  try {
    await invoke('reveal_in_folder', { path: filePath })
  } catch (err) {
    console.error('Failed to reveal in folder:', err)
  }
}

onMounted(async () => {
  const savedPath = localStorage.getItem('downloadPath')
  if (savedPath) downloadPath.value = savedPath

  // Restore persisted queue
  try {
    const saved = localStorage.getItem('downloadQueue')
    if (saved) {
      const parsed = JSON.parse(saved)
      queue.value = parsed.map(item => ({
        ...item,
        // Reset transient UI state
        showFormats: false,
        loadingFormats: false,
        formatTable: [],
        formatsError: null,
        // Downloads that were active when app closed become paused (resumable)
        status: item.status === 'downloading' || item.status === 'fetching'
          ? 'paused'
          : item.status,
      }))
      idCounter = Math.max(0, ...queue.value.map(i => parseInt(i.id) || 0))
    }
  } catch {}

  // Collect format output lines per item (keyed by last added item at time of format fetch)
  // We use a shared buffer since format fetches are sequential
  await listen('format-output', (event) => {
    // Find the item that is currently loading formats
    const loadingItem = queue.value.find(i => i.loadingFormats)
    if (loadingItem) {
      formatOutputBuffer[loadingItem.id] = (formatOutputBuffer[loadingItem.id] || '') + event.payload + '\n'
      if (/^\d+\s+(mhtml|mp4|webm|m4a|opus|aac)/.test(event.payload)) {
        parseFormatTable(loadingItem)
      }
      if (event.payload.includes('SUCCESS')) {
        parseFormatTable(loadingItem)
      }
    }
  })

  await listen('download-progress', (event) => {
    const { download_id, progress, status } = event.payload
    const item = queue.value.find(i => i.id === download_id)
    if (!item) return
    if (progress !== undefined && progress !== null) item.progress = progress
    if (status) item.statusText = status
  })

  await listen('download-complete', (event) => {
    const { download_id, file_path } = event.payload
    const item = queue.value.find(i => i.id === download_id)
    if (item) {
      item.status = 'completed'
      item.progress = 100
      item.statusText = ''
      item.filePath = file_path || null
      saveToHistory(item, file_path)
    }
    startNextIfIdle()
  })

  await listen('download-error', (event) => {
    const { download_id, message } = event.payload
    const item = queue.value.find(i => i.id === download_id)
    // If item is paused, ignore — pauseItem already set the state
    if (!item || item.status === 'paused') return
    const msg = String(message || '')
    if (!msg.includes('paused') && !msg.includes('cancelled')) {
      item.status = 'error'
      item.error = msg
      startNextIfIdle()
    }
  })
})
</script>
