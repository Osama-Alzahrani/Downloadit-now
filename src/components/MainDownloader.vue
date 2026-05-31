<template>
  <!-- Input Section -->
  <div class="bg-gray-800/30 border border-gray-700/50 rounded-lg p-4 mb-4">
    <!-- URL Input -->
    <label class="block text-xs font-semibold text-gray-300 mb-2 uppercase tracking-wider">Video URL</label>
    <div class="flex gap-3 mb-3">
      <input
        v-model="url"
        type="text"
        placeholder="Paste YouTube link here..."
        class="flex-1 px-3 py-2 bg-gray-800 border border-gray-700 text-gray-100 rounded-lg text-sm transition-all placeholder:text-gray-500 focus:outline-none focus:border-teal-500 focus:shadow-lg focus:shadow-teal-500/20"
      />
      <button @click="download" :disabled="loading" class="px-4 py-2 bg-linear-to-r from-teal-500 to-teal-600 hover:from-teal-600 hover:to-teal-700 text-white font-bold rounded-lg transition-all disabled:opacity-50 disabled:cursor-not-allowed text-sm whitespace-nowrap">
        <span v-if="!loading">📥 Download</span>
        <span v-else class="flex items-center gap-1">
          <span class="spinner-sm"></span>
        </span>
      </button>
      <button @click="getFormats" :disabled="loading" class="px-4 py-2 bg-gray-600 hover:bg-gray-700 text-white font-bold rounded-lg transition-all disabled:opacity-50 disabled:cursor-not-allowed text-sm whitespace-nowrap">
        <span v-if="!loading">📊 Formats</span>
        <span v-else class="flex items-center gap-1">
          <span class="spinner-sm"></span>
        </span>
      </button>
    </div>

    <!-- Download Path -->
    <label class="block text-xs font-semibold text-gray-300 mb-2 uppercase tracking-wider">Save Location</label>
    <div class="flex gap-3">
      <div class="flex-1 px-3 py-2 bg-gray-800 border border-gray-700 text-gray-300 rounded-lg text-sm flex items-center">
        <span class="text-xs">{{ downloadPath || '📁 Desktop (default)' }}</span>
      </div>
      <button @click="pickDownloadPath" class="px-4 py-2 bg-gray-600 hover:bg-gray-700 text-white font-bold rounded-lg transition-all text-sm whitespace-nowrap">
        📂 Browse
      </button>
    </div>

    <!-- Status Message (Inline) -->
    <div
      v-if="status"
      :class="[
        'mt-3 p-2 rounded text-xs font-medium transition-all',
        statusType === 'success' ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/50' :
        statusType === 'error' ? 'bg-orange-500/20 text-orange-400 border border-orange-500/50' :
        'bg-teal-500/20 text-teal-400 border border-teal-500/50'
      ]"
    >
      {{ status }}
    </div>

    <!-- Download Progress Bar -->
    <div v-if="(loading || isPaused) && downloadProgress >= 0" class="mt-3">
      <div class="flex justify-between items-center mb-1">
        <span class="text-xs font-semibold text-gray-300">{{ isPaused ? 'Paused' : 'Downloading' }}...</span>
        <span class="text-xs text-gray-400">{{ Math.round(downloadProgress) }}%</span>
      </div>
      <div class="w-full bg-gray-700 rounded-full h-2 overflow-hidden">
        <div
          :class="[
            'h-full transition-all duration-300',
            isPaused
              ? 'bg-linear-to-r from-gray-500 to-gray-600'
              : 'bg-linear-to-r from-teal-500 to-cyan-500'
          ]"
          :style="{ width: `${downloadProgress}%` }"
        ></div>
      </div>
      <p v-if="downloadStatus" class="text-xs text-gray-400 mt-1 truncate">{{ downloadStatus }}</p>
      <div class="flex gap-2 mt-2">
        <button
          v-if="!isPaused"
          @click="pauseDownload"
          class="flex-1 px-3 py-1 bg-yellow-500/20 hover:bg-yellow-500/30 text-yellow-400 text-xs font-semibold rounded transition-colors border border-yellow-500/50"
        >
          ⏸ Pause
        </button>
        <button
          v-else
          @click="resumeDownload"
          class="flex-1 px-3 py-1 bg-teal-500/20 hover:bg-teal-500/30 text-teal-400 text-xs font-semibold rounded transition-colors border border-teal-500/50"
        >
          ▶ Resume
        </button>
        <button
          @click="cancelDownload"
          class="flex-1 px-3 py-1 bg-red-500/20 hover:bg-red-500/30 text-red-400 text-xs font-semibold rounded transition-colors border border-red-500/50"
        >
          ✕ Cancel
        </button>
      </div>
    </div>
  </div>

  <!-- Live Output (while loading) -->
  <div v-if="showFormats && loading" class="flex-1 bg-gray-900/50 border border-gray-700 rounded-lg p-4 mb-4 overflow-hidden flex flex-col">
    <h3 class="text-xs font-semibold text-red-500 mb-2">⚙️ Live Output</h3>
    <pre class="flex-1 text-xs text-gray-400 overflow-y-auto font-mono whitespace-pre-wrap wrap-break-word">{{ liveOutput }}</pre>
  </div>

  <!-- Format Table -->
  <div v-if="showFormats && !loading && hasFormatTable" class="flex-1 flex flex-col overflow-hidden">
    <div class="flex items-center justify-between mb-2">
      <button @click="showFilters = !showFilters" :class="['text-xs px-2 py-1 rounded transition-colors flex items-center gap-2', activeFilterCount > 0 ? 'bg-teal-600 hover:bg-teal-700 text-white' : 'bg-slate-600 hover:bg-slate-700 text-white']">
        {{ showFilters ? '✕ Hide' : '⚙️ Filters' }}
        <span v-if="activeFilterCount > 0" class="bg-white/30 px-1.5 rounded text-xs font-semibold">{{ activeFilterCount }}</span>
      </button>
      <button @click="showFormats = false" class="text-xs text-gray-400 hover:text-gray-200 transition-colors">✕ Close</button>
    </div>

    <!-- Filters Panel -->
    <div v-if="showFilters" class="mb-2 bg-gray-800/30 border border-gray-700/50 rounded p-2">
      <div class="grid grid-cols-3 gap-3 text-xs">
        <!-- Extension Filter -->
        <div>
          <p class="font-semibold text-gray-300 mb-2">Extension</p>
          <div class="space-y-1">
            <label v-for="ext in uniqueExts" :key="ext" class="flex items-center gap-2 cursor-pointer hover:bg-gray-700/30 px-2 py-1 rounded">
              <input
                v-model="filterExt"
                type="checkbox"
                :value="ext"
                class="w-3 h-3 rounded"
              />
              <span class="text-gray-300">{{ ext }}</span>
            </label>
          </div>
        </div>

        <!-- Video Codec Filter -->
        <div>
          <p class="font-semibold text-gray-300 mb-2">Video Codec</p>
          <div class="space-y-1 max-h-40 overflow-y-auto">
            <label v-for="codec in uniqueVCodecs" :key="codec" class="flex items-center gap-2 cursor-pointer hover:bg-gray-700/30 px-2 py-1 rounded">
              <input
                v-model="filterVCodec"
                type="checkbox"
                :value="codec"
                class="w-3 h-3 rounded"
              />
              <span class="text-gray-300 text-xs truncate">{{ codec }}</span>
            </label>
          </div>
        </div>

        <!-- Audio Codec Filter -->
        <div>
          <p class="font-semibold text-gray-300 mb-2">Audio Codec</p>
          <div class="space-y-1 max-h-40 overflow-y-auto">
            <label v-for="codec in uniqueACodecs" :key="codec" class="flex items-center gap-2 cursor-pointer hover:bg-gray-700/30 px-2 py-1 rounded">
              <input
                v-model="filterACodec"
                type="checkbox"
                :value="codec"
                class="w-3 h-3 rounded"
              />
              <span class="text-gray-300 text-xs truncate">{{ codec }}</span>
            </label>
          </div>
        </div>
      </div>

      <!-- Clear Filters -->
      <button
        v-if="filterExt.length || filterVCodec.length || filterACodec.length"
        @click="filterExt = []; filterVCodec = []; filterACodec = []"
        class="w-full mt-2 text-xs bg-gray-700 hover:bg-gray-600 text-gray-300 px-2 py-1 rounded transition-colors"
      >
        Clear All Filters
      </button>
    </div>

    <DataTable
      title="📊 Available Formats"
      :columns="tableColumns"
      :rows="formattedTable"
      :searchable="true"
      :searchFields="['id', 'ext', 'resolution', 'vcodec', 'acodec']"
      :filters="tableFilters"
    >
      <template #cell-id="{ value, row }">
        <span :class="['font-bold', 'text-blue-400']">{{ value }}</span>
      </template>
      <template #cell-ext="{ value }">
        <span class="font-semibold text-blue-300">{{ value }}</span>
      </template>
      <template #cell-filesize="{ value }">
        <span class="text-emerald-400 font-mono text-xs">{{ value }}</span>
      </template>
      <template #cell-tbr="{ value }">
        <span class="text-amber-400 font-mono text-xs">{{ value }}</span>
      </template>
      <template #cell-proto="{ value }">
        <span class="text-cyan-300 text-xs font-semibold">{{ value }}</span>
      </template>
      <template #cell-vcodec="{ value }">
        <span class="font-mono text-cyan-200 text-xs">{{ value }}</span>
      </template>
      <template #cell-acodec="{ value }">
        <span class="font-mono text-pink-200 text-xs">{{ value }}</span>
      </template>
      <template #cell-action="{ row }">
        <button
          @click="downloadFormat(row.id)"
          :disabled="loading"
          class="px-2 py-1 bg-linear-to-r from-teal-500 to-teal-600 hover:from-teal-600 hover:to-teal-700 text-white text-xs font-semibold rounded transition-all disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap"
        >
          {{ loading ? '...' : '↓' }}
        </button>
      </template>
    </DataTable>

    <!-- Format Tips -->
    <div class="mt-2 grid grid-cols-3 gap-2">
      <div v-if="quickFormat" class="bg-yellow-500/15 border border-yellow-500/30 rounded p-2 cursor-pointer hover:bg-yellow-500/25 transition-colors">
        <p class="text-xs text-yellow-400 font-semibold">💡 Quick</p>
        <p class="text-xs text-yellow-300">ID <span class="font-bold">{{ quickFormat.id }}</span></p>
        <p class="text-xs text-yellow-300/70">{{ quickFormat.resolution }} • {{ quickFormat.filesize }}</p>
      </div>
      <div v-else class="bg-gray-500/15 border border-gray-500/30 rounded p-2">
        <p class="text-xs text-gray-400 font-semibold">💡 Quick</p>
        <p class="text-xs text-gray-400">N/A</p>
      </div>

      <div v-if="bestFormat" class="bg-cyan-500/15 border border-cyan-500/30 rounded p-2 cursor-pointer hover:bg-cyan-500/25 transition-colors">
        <p class="text-xs text-cyan-400 font-semibold">🎬 Best</p>
        <p class="text-xs text-cyan-300">ID <span class="font-bold">{{ bestFormat.id }}</span></p>
        <p class="text-xs text-cyan-300/70">{{ bestFormat.resolution }} • {{ bestFormat.tbr }}</p>
      </div>
      <div v-else class="bg-gray-500/15 border border-gray-500/30 rounded p-2">
        <p class="text-xs text-gray-400 font-semibold">🎬 Best</p>
        <p class="text-xs text-gray-400">N/A</p>
      </div>

      <div v-if="audioFormat" class="bg-purple-500/15 border border-purple-500/30 rounded p-2 cursor-pointer hover:bg-purple-500/25 transition-colors">
        <p class="text-xs text-purple-400 font-semibold">🎵 Audio</p>
        <p class="text-xs text-purple-300">ID <span class="font-bold">{{ audioFormat.id }}</span></p>
        <p class="text-xs text-purple-300/70">{{ audioFormat.ext }} • {{ audioFormat.tbr }}</p>
      </div>
      <div v-else class="bg-gray-500/15 border border-gray-500/30 rounded p-2">
        <p class="text-xs text-gray-400 font-semibold">🎵 Audio</p>
        <p class="text-xs text-gray-400">N/A</p>
      </div>
    </div>
  </div>

  <!-- Raw Format Display (fallback) -->
  <div v-if="showFormats && !loading && formats && !hasFormatTable" class="flex-1 flex flex-col overflow-hidden">
    <div class="flex items-center justify-between mb-2">
      <h3 class="text-sm font-bold text-red-500">Available Formats</h3>
      <button @click="showFormats = false" class="text-xs text-gray-400 hover:text-gray-200 transition-colors">✕ Close</button>
    </div>
    <div class="flex-1 bg-gray-900/50 border border-gray-700 rounded-lg p-4 overflow-hidden">
      <pre class="text-xs text-gray-400 overflow-y-auto font-mono h-full">{{ formats }}</pre>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import DataTable from './DataTable.vue'

const url = ref('')
const loading = ref(false)
const status = ref('')
const statusType = ref('')
const formats = ref('')
const showFormats = ref(false)
const liveOutput = ref('')
const formatTable = ref([])
const hasFormatTable = ref(false)
const downloadPath = ref(null)
const downloadProgress = ref(0)
const downloadStatus = ref('')
const isPaused = ref(false)
const pausedParams = ref(null)

const filterExt = ref([])
const filterVCodec = ref([])
const filterACodec = ref([])
const showFilters = ref(false)

const tableFilters = computed(() => ({
  ext: { values: filterExt.value, matchAny: true },
  vcodec: { values: filterVCodec.value, matchAny: true },
  acodec: { values: filterACodec.value, matchAny: true }
}))

const uniqueExts = computed(() => [...new Set(formatTable.value.map(f => f.ext))].filter(Boolean).sort())
const uniqueVCodecs = computed(() => [...new Set(formatTable.value.map(f => f.vcodec))].filter(v => v !== '-').sort())
const uniqueACodecs = computed(() => [...new Set(formatTable.value.map(f => f.acodec))].filter(v => v !== '-').sort())

const activeFilterCount = computed(() => {
  return filterExt.value.length + filterVCodec.value.length + filterACodec.value.length
})

const quickFormat = computed(() => {
  // Find a small, complete format (video + audio, not too large)
  const candidates = formatTable.value.filter(f =>
    f.acodec !== '-' && f.vcodec !== '-' && f.resolution !== '-' && f.resolution !== 'audio'
  )
  if (candidates.length === 0) return null
  // Sort by filesize and pick a reasonable one
  return candidates.sort((a, b) => {
    const aSize = parseFloat(a.filesize) || 0
    const bSize = parseFloat(b.filesize) || 0
    return aSize - bSize
  })[0]
})

const bestFormat = computed(() => {
  // Find highest resolution video
  const videoFormats = formatTable.value.filter(f =>
    f.vcodec !== '-' && f.resolution !== '-' && f.resolution !== 'audio'
  )
  if (videoFormats.length === 0) return null
  return videoFormats.sort((a, b) => {
    const aRes = parseInt(a.resolution) || 0
    const bRes = parseInt(b.resolution) || 0
    return bRes - aRes
  })[0]
})

const audioFormat = computed(() => {
  // Find best audio-only format
  const audioFormats = formatTable.value.filter(f =>
    f.resolution === 'audio' || (f.acodec !== '-' && f.vcodec === '-')
  )
  if (audioFormats.length === 0) return null
  return audioFormats.sort((a, b) => {
    const aBit = parseFloat(a.tbr) || 0
    const bBit = parseFloat(b.tbr) || 0
    return bBit - aBit
  })[0]
})

// Computed property for format table with row classes
const formattedTable = computed(() => {
  return formatTable.value.map(f => {
    let rowClass = undefined
    if (quickFormat.value && f.id === quickFormat.value.id) {
      rowClass = 'row-quick'
    } else if (bestFormat.value && f.id === bestFormat.value.id) {
      rowClass = 'row-best'
    } else if (audioFormat.value && f.id === audioFormat.value.id) {
      rowClass = 'row-audio'
    }
    return { ...f, _rowClass: rowClass }
  })
})

const tableColumns = [
  { key: 'id', label: 'ID', width: '50px', sortable: true, sortValue: (v) => parseInt(v) || 0 },
  { key: 'ext', label: 'Ext', width: '60px', sortable: true },
  { key: 'resolution', label: 'Resolution', width: '100px', sortable: true },
  {
    key: 'fps',
    label: 'FPS',
    width: '45px',
    sortable: true,
    sortValue: (v) => parseInt(v) || 0
  },
  {
    key: 'ch',
    label: 'Ch',
    width: '45px',
    sortable: true,
    sortValue: (v) => parseInt(v) || 0
  },
  {
    key: 'filesize',
    label: 'Size',
    width: '90px',
    sortable: true,
    sortValue: (v) => {
      const match = String(v).match(/[\d.]+/)
      return parseFloat(match?.[0] || 0)
    }
  },
  {
    key: 'tbr',
    label: 'Bitrate',
    width: '80px',
    sortable: true,
    sortValue: (v) => {
      const match = String(v).match(/[\d.]+/)
      return parseFloat(match?.[0] || 0)
    }
  },
  { key: 'proto', label: 'Proto', width: '60px', sortable: true },
  { key: 'vcodec', label: 'Video', sortable: true },
  { key: 'acodec', label: 'Audio', sortable: true },
  { key: 'action', label: 'Action', width: '70px', sortable: false }
]

onMounted(async () => {
  // Restore download path from localStorage
  const savedPath = localStorage.getItem('downloadPath')
  if (savedPath) {
    downloadPath.value = savedPath
  }

  await listen('format-output', (event) => {
    liveOutput.value += event.payload + '\n'
    if (/^\d+\s+(mhtml|mp4|webm|m4a)/.test(event.payload)) {
      setTimeout(() => parseFormatTable(), 100)
    }
    if (event.payload.includes('SUCCESS')) {
      setTimeout(() => parseFormatTable(), 300)
    }
  })

  await listen('download-progress', (event) => {
    const data = event.payload
    if (data.progress !== undefined && data.progress !== null) {
      downloadProgress.value = data.progress
      console.log('📊 Progress updated:', data.progress)
    }
    if (data.status) {
      downloadStatus.value = data.status
    }
  })

  await listen('download-complete', (event) => {
    downloadProgress.value = 100
    status.value = event.payload
    statusType.value = 'success'
    loading.value = false
    url.value = ''
  })

  await listen('download-error', (event) => {
    const errorMsg = event.payload
    // Don't reset progress if it's a pause/cancel error
    if (!errorMsg.includes('paused') && !errorMsg.includes('cancelled')) {
      downloadProgress.value = 0
    }
    status.value = `❌ ${errorMsg}`
    statusType.value = 'error'
    loading.value = false
  })
})

const pickDownloadPath = async () => {
  try {
    const selected = await open({ directory: true, multiple: false })
    if (selected) {
      downloadPath.value = selected
      localStorage.setItem('downloadPath', selected)
      status.value = `📁 Download path set to: ${selected}`
      statusType.value = 'success'
    }
  } catch (error) {
    status.value = `❌ Failed to pick folder: ${error}`
    statusType.value = 'error'
  }
}

const pauseDownload = async () => {
  try {
    // Save current progress and params for resume
    const pausedProgress = downloadProgress.value
    const pausedStatus = downloadStatus.value
    if (pausedParams.value) {
      pausedParams.value.url = url.value
    }
    const result = await invoke('pause_download')

    // Restore progress after pausing
    downloadProgress.value = pausedProgress
    downloadStatus.value = pausedStatus

    status.value = `⏸ Download paused`
    statusType.value = 'success'
    loading.value = false
    isPaused.value = true
  } catch (error) {
    status.value = `❌ ${error}`
    statusType.value = 'error'
  }
}

const resumeDownload = async () => {
  if (!pausedParams.value) {
    status.value = '❌ No paused download to resume'
    statusType.value = 'error'
    return
  }
  isPaused.value = false
  loading.value = true
  downloadProgress.value = 0
  downloadStatus.value = ''
  try {
    await invoke('download_video_stream', { params: pausedParams.value })
  } catch (error) {
    status.value = `❌ ${error}`
    statusType.value = 'error'
    loading.value = false
  }
}

const cancelDownload = async () => {
  try {
    const result = await invoke('cancel_download')
    status.value = `⏹ Download cancelled`
    statusType.value = 'success'
    loading.value = false
    downloadProgress.value = 0
    downloadStatus.value = ''
    isPaused.value = false
    pausedParams.value = null
  } catch (error) {
    status.value = `❌ ${error}`
    statusType.value = 'error'
  }
}

const download = async () => {
  loading.value = true
  downloadProgress.value = 0
  downloadStatus.value = ''
  isPaused.value = false
  const params = { url: url.value, format: null, download_path: downloadPath.value }
  pausedParams.value = { ...params }
  console.log('Download called with:', params)
  try {
    await invoke('download_video_stream', { params })
  } catch (error) {
    status.value = `❌ ${error}`
    statusType.value = 'error'
    loading.value = false
  }
}

const downloadFormat = async (formatId) => {
  loading.value = true
  downloadProgress.value = 0
  downloadStatus.value = ''
  isPaused.value = false
  const params = { url: url.value, format: formatId, download_path: downloadPath.value }
  pausedParams.value = { ...params }
  console.log('Download format called with:', params)
  try {
    await invoke('download_video_stream', { params })
  } catch (error) {
    status.value = `❌ ${error}`
    statusType.value = 'error'
    loading.value = false
  }
}

const getFormats = async () => {
  loading.value = true
  liveOutput.value = ''
  showFormats.value = true
  try {
    const result = await invoke('get_video_formats', { url: url.value })
    formats.value = result
    status.value = ''
  } catch (error) {
    status.value = `❌ ${error}`
    statusType.value = 'error'
  } finally {
    loading.value = false
  }
}

function parseFormatTable() {
  const lines = liveOutput.value.split('\n')
  formatTable.value = []
  let inTable = false

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    const trimmed = line.trim()

    if (!trimmed) continue

    if (trimmed.includes('ID') && trimmed.includes('EXT')) {
      inTable = true
      continue
    }

    if (trimmed.startsWith('---') || trimmed.startsWith('===')) continue
    if (trimmed.includes('WARNING') || trimmed.includes('[info]')) {
      inTable = false
      continue
    }

    if (inTable && trimmed && /^\d+\s+/.test(trimmed)) {
      const sections = line.split('|').map(s => s.trim())
      if (sections.length >= 1) {
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
          filesize = parts[0] || '-'
          tbr = parts[1] || '-'
          proto = parts[2] || '-'
        }

        let vcodec = '-', acodec = '-', moreInfo = '-'
        if (sections.length > 2) {
          const codecPart = sections[2]
          const codecParts = codecPart.split(/\s+/)
          vcodec = codecParts[0] || '-'
          acodec = codecParts[codecParts.length - 1] || '-'
          moreInfo = 'standard'
        }

        formatTable.value.push({
          id,
          ext,
          resolution,
          fps,
          ch,
          filesize,
          tbr,
          proto,
          vcodec,
          acodec,
          moreInfo
        })
      }
    }
  }

  if (formatTable.value.length > 0) {
    hasFormatTable.value = true
  }
}
</script>
