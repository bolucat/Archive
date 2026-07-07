<script setup lang="ts">
import { BookOpen, Grid3X3, List, Search, Settings, Upload, X } from 'lucide-vue-next'
import type { BookManagerSortMode, BookManagerSortOrder, BookViewMode } from '../../types/bookShelf'
import { getManagerHeaderActions, type ManagerHeaderActionKey } from '../../utils/bookManagerActions'

const props = defineProps<{
  collapsed: boolean
  query: string
  placeholder: string
  sortOpen: boolean
  sortModeOptions: Array<{ value: BookManagerSortMode; label: string }>
  effectiveSortMode: BookManagerSortMode
  effectiveSortOrder: BookManagerSortOrder
  viewMode: BookViewMode
}>()

const emit = defineEmits<{
  (e: 'update:query', value: string): void
  (e: 'update:sortOpen', value: boolean): void
  (e: 'change-sort-mode', mode: BookManagerSortMode): void
  (e: 'toggle-sort-order'): void
  (e: 'change-view-mode', mode: BookViewMode): void
  (e: 'open-settings'): void
  (e: 'open-import'): void
  (e: 'scan-cloud'): void
  (e: 'opds'): void
  (e: 'sort'): void
}>()

const actions = getManagerHeaderActions()
const actionByKey = Object.fromEntries(actions.map((action) => [action.key, action])) as Record<ManagerHeaderActionKey, typeof actions[number]>
const quickActions = actions.filter((action) => action.key === 'scan-cloud' || action.key === 'opds')

function trigger(key: ManagerHeaderActionKey) {
  if (key === 'import-local') emit('open-import')
  if (key === 'scan-cloud') emit('scan-cloud')
  if (key === 'opds') emit('opds')
  if (key === 'sort') emit('sort')
  if (key === 'settings') emit('open-settings')
}

function toggleSort() {
  const nextOpen = !props.sortOpen
  emit('update:sortOpen', nextOpen)
  if (nextOpen) trigger('sort')
}
</script>

<template>
  <header class="header" :style="collapsed ? { marginLeft: '40px' } : {}">
    <div class="header-search-container" :style="collapsed ? { width: '369px' } : {}">
      <div style="position:relative">
        <input
          type="text"
          class="header-search-box"
          :value="query"
          :placeholder="placeholder"
          @input="emit('update:query', ($event.target as HTMLInputElement).value)"
        />
        <span class="header-search-text" @click="emit('update:query', '')">
          <Search v-if="!query" :size="22" :stroke-width="1.5" style="opacity:0.6" />
          <X v-else :size="16" :stroke-width="1.8" style="opacity:0.6" />
        </span>
      </div>
    </div>
    <div class="setting-icon-parrent" :style="collapsed ? { marginLeft: '430px' } : {}">
      <button
        v-for="action in quickActions"
        :key="action.key"
        class="manager-action-pill"
        :title="action.title"
        type="button"
        @click="trigger(action.key)"
      >
        {{ action.label }}
      </button>
      <div class="setting-icon-container" :title="actionByKey.sort.title" @click.stop="toggleSort">
        <List :size="22" :stroke-width="1.9" />
        <div v-if="sortOpen" class="header-sort-pop" @click.stop>
          <div
            v-for="opt in sortModeOptions"
            :key="opt.value"
            :class="['sort-pop-item', effectiveSortMode === opt.value ? 'active' : '']"
            @click="emit('change-sort-mode', opt.value)"
          >
            {{ opt.label }}
          </div>
          <div class="sort-pop-item divider" @click="emit('toggle-sort-order')">
            {{ effectiveSortOrder === 'asc' ? '↑ Ascending' : '↓ Descending' }}
          </div>
        </div>
      </div>
      <div class="setting-icon-container" :title="actionByKey['view-mode'].title">
        <Grid3X3 v-if="viewMode === 'grid'" :size="22" :stroke-width="1.9" @click="emit('change-view-mode', 'list')" />
        <List v-else-if="viewMode === 'list'" :size="22" :stroke-width="1.9" @click="emit('change-view-mode', 'cover')" />
        <BookOpen v-else :size="22" :stroke-width="1.9" @click="emit('change-view-mode', 'grid')" />
      </div>
      <div class="setting-icon-container" :title="actionByKey.settings.title" @click="trigger('settings')">
        <Settings :size="22" :stroke-width="1.9" />
      </div>
    </div>
    <div class="import-from-local" :title="actionByKey['import-local'].title" @click="trigger('import-local')">
      <Upload :size="18" :stroke-width="1.9" style="margin-right:6px" />
      <span>Import</span>
      <div class="animation-mask-local"></div>
    </div>
  </header>
</template>

<style scoped>
.header { width:100%; height:80px; overflow:hidden; margin-left:190px; position:relative; border-bottom:1px solid var(--color-border); }
.header-search-container { position:absolute; top:23px; margin-left:40px; width:220px; }
.header-search-box {
  width:calc(100% - 20px); height:39px; border-radius:22px;
  border:1px solid var(--color-border); outline:none; font-size:15px;
  padding:0; padding-left:20px; background:var(--color-bg-1); color:var(--color-text-1);
}
.header-search-box::placeholder { font-size:15px; line-height:39px; text-overflow:ellipsis; white-space:nowrap; overflow:hidden; width:calc(100% - 40px); color:var(--color-text-4); }
.header-search-text { position:absolute; top:0; right:0; font-size:15px; display:inline-block; width:40px; height:100%; cursor:pointer; display:flex; align-items:center; justify-content:center; }
.header-search-text:hover { border-radius:50%; background:var(--color-fill-2); }

.setting-icon-parrent { position:absolute; top:18px; margin-left:280px; height:50px; z-index:1; display:flex; }
.manager-action-pill {
  height:32px;
  min-width:52px;
  margin:9px 2px 0 0;
  padding:0 10px;
  border:1px solid var(--color-border);
  border-radius:18px;
  background:var(--color-bg-2);
  color:var(--color-text-2);
  font-size:13px;
  cursor:pointer;
}
.manager-action-pill:hover {
  background:var(--color-fill-2);
  color:rgb(var(--primary-6));
}
.setting-icon-container { width:50px; height:50px; cursor:pointer; display:flex; align-items:center; justify-content:center; margin-left:15px; color:var(--color-text-2); position:relative; }
.setting-icon-container:hover { border-radius:50%; background:var(--color-fill-2); }

.header-sort-pop { position:absolute; top:50px; left:0; min-width:160px; background:var(--color-bg-3); border:1px solid var(--color-border); border-radius:8px; padding:4px; z-index:20; box-shadow:0 4px 16px rgba(0,0,0,.12); }
.sort-pop-item { padding:6px 12px; font-size:13px; cursor:pointer; border-radius:4px; color:var(--color-text-1); }
.sort-pop-item:hover { background:var(--color-fill-2); }
.sort-pop-item.active { color:rgb(var(--primary-6)); font-weight:600; }
.sort-pop-item.divider { border-top:1px solid var(--color-border); margin-top:4px; padding-top:8px; }

.import-from-local {
  position:absolute; right:25px; top:23px; width:138px; height:42px;
  border-radius:25px; cursor:pointer; z-index:5; font-size:15px; font-weight:500;
  display:flex; align-items:center; justify-content:center;
  color:var(--color-text-1); background:var(--color-bg-2); border:1px solid var(--color-border); overflow:hidden;
}
.animation-mask-local { width:100%; height:100%; position:absolute; top:0; left:0; transition:.1s; transform:scaleX(0); transform-origin:left; border-radius:19px; background:rgba(var(--primary-6),.08); }
.import-from-local:hover .animation-mask-local { transform:scaleX(1); }
</style>
