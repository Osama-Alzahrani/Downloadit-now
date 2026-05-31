<template>
  <div class="data-table-container">
    <div class="data-table-header">
      <h3 class="data-table-title">{{ title }} <span v-if="rows.length" class="data-table-count">({{ rows.length }})</span></h3>
      <div class="data-table-controls">
        <input
          v-if="searchable"
          v-model="searchQuery"
          type="text"
          placeholder="Search..."
          class="data-table-search"
        />
        <slot name="controls"></slot>
      </div>
    </div>

    <div class="data-table-wrapper">
      <table class="data-table">
        <thead>
          <tr>
            <th
              v-for="col in columns"
              :key="col.key"
              :style="{ width: col.width }"
              :class="{ sortable: col.sortable, sorted: sortKey === col.key }"
              @click="col.sortable && toggleSort(col.key)"
            >
              {{ col.label }}
              <span v-if="col.sortable && sortKey === col.key" class="sort-indicator">
                {{ sortOrder === 'asc' ? '▲' : '▼' }}
              </span>
            </th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(row, idx) in filteredAndSortedRows"
            :key="idx"
            :class="[
              { recommended: row._recommended, error: row._error, success: row._success },
              row._rowClass
            ]"
          >
            <td
              v-for="col in columns"
              :key="col.key"
              :class="col.class"
            >
              <slot :name="`cell-${col.key}`" :value="row[col.key]" :row="row">
                {{ formatCellValue(row[col.key], col.format) }}
              </slot>
            </td>
          </tr>
          <tr v-if="!filteredAndSortedRows.length" class="empty-row">
            <td :colspan="columns.length" class="empty-message">No data available</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'

const props = defineProps({
  title: {
    type: String,
    default: 'Data'
  },
  columns: {
    type: Array,
    required: true
    // Each column: { key, label, width?, sortable?, class?, format?, sortValue? }
    // sortValue: function to extract sortable value (e.g., "128k" -> 128)
  },
  rows: {
    type: Array,
    required: true
  },
  searchable: {
    type: Boolean,
    default: true
  },
  searchFields: {
    type: Array,
    default: () => []
  },
  filters: {
    type: Object,
    default: () => ({})
    // { fieldName: { values: [], matchAny: true } }
  }
})

const sortKey = ref(null)
const sortOrder = ref('asc')
const searchQuery = ref('')

const toggleSort = (key) => {
  if (sortKey.value === key) {
    sortOrder.value = sortOrder.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortKey.value = key
    sortOrder.value = 'asc'
  }
}

const filteredAndSortedRows = computed(() => {
  let result = [...props.rows]

  // Apply field filters
  Object.entries(props.filters).forEach(([field, filterConfig]) => {
    if (filterConfig.values && filterConfig.values.length > 0) {
      if (filterConfig.matchAny) {
        result = result.filter(row =>
          filterConfig.values.includes(String(row[field]))
        )
      } else {
        result = result.filter(row =>
          filterConfig.values.every(val => String(row[field]).includes(val))
        )
      }
    }
  })

  // Filter by search
  if (searchQuery.value && props.searchFields.length > 0) {
    const query = searchQuery.value.toLowerCase()
    result = result.filter(row =>
      props.searchFields.some(field =>
        String(row[field]).toLowerCase().includes(query)
      )
    )
  }

  // Sort
  if (sortKey.value) {
    const col = props.columns.find(c => c.key === sortKey.value)
    result.sort((a, b) => {
      let aVal = a[sortKey.value]
      let bVal = b[sortKey.value]

      // Use custom sort value extractor if provided
      if (col?.sortValue) {
        aVal = col.sortValue(aVal)
        bVal = col.sortValue(bVal)
      }

      // Handle numeric sorting
      if (typeof aVal === 'number' && typeof bVal === 'number') {
        return sortOrder.value === 'asc' ? aVal - bVal : bVal - aVal
      }

      // Handle string sorting
      aVal = String(aVal).toLowerCase()
      bVal = String(bVal).toLowerCase()
      const cmp = aVal.localeCompare(bVal)
      return sortOrder.value === 'asc' ? cmp : -cmp
    })
  }

  return result
})

const formatCellValue = (value, format) => {
  if (!format) return value
  if (format === 'number') return Number(value).toLocaleString()
  if (format === 'currency') return `$${Number(value).toFixed(2)}`
  if (typeof format === 'function') return format(value)
  return value
}
</script>

<style scoped>
.data-table-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: rgba(17, 24, 39, 0.5);
  border: 1px solid rgba(55, 65, 81, 0.5);
  border-radius: 0.5rem;
  overflow: hidden;
}

.data-table-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid rgba(55, 65, 81, 0.3);
  background: rgba(31, 41, 55, 0.5);
}

.data-table-title {
  margin: 0;
  font-size: 0.875rem;
  font-weight: 700;
  color: #e5e7eb;
}

.data-table-count {
  font-size: 0.75rem;
  color: #9ca3af;
  font-weight: 400;
}

.data-table-controls {
  display: flex;
  gap: 0.5rem;
  align-items: center;
}

.data-table-search {
  padding: 0.375rem 0.75rem;
  font-size: 0.75rem;
  background: rgba(17, 24, 39, 0.8);
  border: 1px solid rgba(75, 85, 99, 0.5);
  border-radius: 0.375rem;
  color: #e5e7eb;
  transition: all 0.2s;
}

.data-table-search:focus {
  outline: none;
  border-color: rgba(20, 184, 166, 0.5);
  box-shadow: 0 0 0 3px rgba(20, 184, 166, 0.1);
}

.data-table-wrapper {
  flex: 1;
  overflow: auto;
}

.data-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.8125rem;
}

.data-table thead {
  background: linear-gradient(135deg, #0d5f57 0%, #0d7a70 100%);
  border-bottom: 2px solid #14b8a6;
  position: sticky;
  top: 0;
  z-index: 10;
  opacity: 1;
}

.data-table th {
  padding: 0.5rem 0.75rem;
  text-align: left;
  font-weight: 700;
  font-size: 0.7rem;
  color: #5eead4;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  user-select: none;
}

.data-table th.sortable {
  cursor: pointer;
  transition: background-color 0.2s;
}

.data-table th.sortable:hover {
  background-color: rgba(20, 184, 166, 0.15);
}

.sort-indicator {
  margin-left: 0.25rem;
  font-size: 0.6rem;
  opacity: 0.6;
}

.data-table td {
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid rgba(107, 114, 128, 0.2);
  color: #d1d5db;
}

.data-table tbody tr {
  transition: all 0.2s ease;
  background-color: transparent;
}

.data-table tbody tr:hover {
  background-color: rgba(20, 184, 166, 0.08);
  border-left: 3px solid rgba(20, 184, 166, 0.5);
  padding-left: 0;
}

.data-table tbody tr.recommended {
  background-color: rgba(34, 197, 94, 0.08);
  border-left: 3px solid rgba(34, 197, 94, 0.6);
}

.data-table tbody tr.recommended:hover {
  background-color: rgba(34, 197, 94, 0.15);
  border-left-color: rgba(34, 197, 94, 0.8);
}

.data-table tbody tr.error {
  background-color: rgba(239, 68, 68, 0.08);
}

.data-table tbody tr.success {
  background-color: rgba(34, 197, 94, 0.08);
}

.empty-row {
  height: 3rem;
}

.empty-message {
  text-align: center;
  color: #6b7280;
  font-style: italic;
}

.data-table tbody tr.row-quick {
  background-color: rgba(234, 179, 8, 0.2);
  border-left: 4px solid #eab308;
  box-shadow: inset 0 0 15px rgba(234, 179, 8, 0.2);
}

.data-table tbody tr.row-quick:hover {
  background-color: rgba(234, 179, 8, 0.3);
  border-left-color: #facc15;
  box-shadow: inset 0 0 25px rgba(234, 179, 8, 0.3);
}

.data-table tbody tr.row-best {
  background-color: rgba(6, 182, 212, 0.2);
  border-left: 4px solid #06b6d4;
  box-shadow: inset 0 0 15px rgba(6, 182, 212, 0.2);
}

.data-table tbody tr.row-best:hover {
  background-color: rgba(6, 182, 212, 0.3);
  border-left-color: #22d3ee;
  box-shadow: inset 0 0 25px rgba(6, 182, 212, 0.3);
}

.data-table tbody tr.row-audio {
  background-color: rgba(168, 85, 247, 0.2);
  border-left: 4px solid #a855f7;
  box-shadow: inset 0 0 15px rgba(168, 85, 247, 0.2);
}

.data-table tbody tr.row-audio:hover {
  background-color: rgba(168, 85, 247, 0.3);
  border-left-color: #d8b4fe;
  box-shadow: inset 0 0 25px rgba(168, 85, 247, 0.3);
}
</style>
