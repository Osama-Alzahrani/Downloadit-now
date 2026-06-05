<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <!-- Toolbar -->
    <div class="flex items-center justify-between mb-3 px-1">
      <span class="text-xs text-gray-500">{{ history.length }} download{{ history.length !== 1 ? 's' : '' }}</span>
      <button
        v-if="history.length > 0"
        @click="clearHistory"
        class="px-2.5 py-1 bg-gray-700 hover:bg-red-500/20 border border-gray-600 hover:border-red-500/40 text-gray-400 hover:text-red-400 text-xs rounded transition-colors"
      >
        Clear All
      </button>
    </div>

    <!-- Empty state -->
    <div v-if="history.length === 0" class="flex flex-col items-center justify-center flex-1 text-gray-600 py-16">
      <img src="../assets/downloadit-glyph.svg" alt="" class="mb-3 opacity-40" />
      <p class="text-sm font-medium mb-1">No download history</p>
      <p class="text-xs">Completed downloads will appear here</p>
    </div>

    <!-- History list -->
    <div v-else class="flex-1 overflow-y-auto space-y-2 min-h-0">
      <div
        v-for="entry in history"
        :key="entry.id"
        class="bg-gray-800/40 border border-gray-700/40 rounded-lg p-3 flex items-center gap-3"
      >
        <!-- Thumbnail -->
        <div class="shrink-0">
          <img
            v-if="entry.thumbnail"
            :src="entry.thumbnail"
            class="w-20 h-12 object-cover rounded"
            @error="e => e.target.style.display = 'none'"
          />
          <div v-else class="w-20 h-12 bg-gray-700 rounded flex items-center justify-center text-2xl">🎬</div>
        </div>

        <!-- Info -->
        <div class="flex-1 min-w-0">
          <p class="text-sm font-semibold text-gray-100 leading-tight truncate mb-1" :title="entry.title">{{ entry.title }}</p>
          <div class="flex items-center gap-1.5 flex-wrap mb-1.5">
            <span v-if="entry.ext" class="px-1.5 py-0.5 bg-blue-500/25 border border-blue-500/40 text-blue-300 text-xs rounded font-mono uppercase">{{ entry.ext }}</span>
            <span v-if="entry.width && entry.height" class="px-1.5 py-0.5 bg-gray-700/80 text-gray-300 text-xs rounded font-mono">{{ entry.width }}×{{ entry.height }}</span>
            <span v-if="entry.selectedFormatLabel" class="px-1.5 py-0.5 bg-teal-500/20 border border-teal-500/40 text-teal-300 text-xs rounded">{{ entry.selectedFormatLabel }}</span>
            <span class="text-xs text-gray-500">{{ formatDate(entry.downloadedAt) }}</span>
          </div>
          <div class="flex items-center gap-2">
            <button
              v-if="entry.filePath"
              @click="openFile(entry.filePath)"
              class="px-1.5 py-0.5 bg-emerald-500/15 hover:bg-emerald-500/30 border border-emerald-500/30 text-emerald-400 text-[10px] font-semibold rounded transition-colors"
            >▶ Open</button>
            <button
              v-if="entry.filePath"
              @click="openFolder(entry.filePath)"
              class="px-1.5 py-0.5 bg-gray-700/60 hover:bg-gray-600/60 text-gray-400 text-[10px] font-semibold rounded transition-colors"
            >📂 Folder</button>
            <span
              v-if="entry.filePath"
              class="text-[10px] text-gray-600 truncate"
              :title="entry.filePath"
            >{{ entry.filePath }}</span>
          </div>
        </div>

        <!-- Remove -->
        <button @click="deleteEntry(entry.id)" class="shrink-0 text-gray-600 hover:text-gray-400 text-xl leading-none transition-colors">×</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const emit = defineEmits(['history-update'])

const history = ref([])

const loadHistory = () => {
  try {
    history.value = JSON.parse(localStorage.getItem('downloadHistory') || '[]')
  } catch {
    history.value = []
  }
}

const clearHistory = () => {
  localStorage.removeItem('downloadHistory')
  history.value = []
  emit('history-update', 0)
}

const deleteEntry = (id) => {
  history.value = history.value.filter(e => e.id !== id)
  localStorage.setItem('downloadHistory', JSON.stringify(history.value))
  emit('history-update', history.value.length)
}

const openFile = async (filePath) => {
  try { await invoke('open_file', { path: filePath }) } catch {}
}

const openFolder = async (filePath) => {
  try { await invoke('reveal_in_folder', { path: filePath }) } catch {}
}

const formatDate = (iso) => {
  if (!iso) return ''
  const d = new Date(iso)
  const now = new Date()
  const diffDays = Math.floor((now - d) / 86400000)
  if (diffDays === 0) return 'Today ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  if (diffDays === 1) return 'Yesterday'
  if (diffDays < 7) return `${diffDays} days ago`
  return d.toLocaleDateString()
}

onMounted(loadHistory)
</script>
