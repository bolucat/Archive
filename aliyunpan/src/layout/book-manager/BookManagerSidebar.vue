<script setup lang="ts">
import {
  BarChart3, Bookmark, ChevronDown, CloudDownload, Highlighter,
  Home, LibraryBig, Menu, Pencil, Plus, Star, StickyNote, Trash2,
} from 'lucide-vue-next'
import type { BookManagerView, BookShelfGroup } from '../../types/bookShelf'

defineProps<{
  tabs: Array<{ key: BookManagerView; label: string }>
  activeView: BookManagerView
  collapsed: boolean
  shelfCollapsed: boolean
  shelves: BookShelfGroup[]
  activeShelfTitle: string
  isScanning: boolean
  totalCount: number
}>()

const emit = defineEmits<{
  (e: 'update:collapsed', value: boolean): void
  (e: 'update:shelfCollapsed', value: boolean): void
  (e: 'open-view', view: BookManagerView): void
  (e: 'open-shelf', shelf: BookShelfGroup): void
  (e: 'scan-now'): void
  (e: 'clear-library'): void
  (e: 'show-stats'): void
}>()
</script>

<template>
  <aside :class="['sidebar', collapsed ? 'collapsed' : '']">
    <div class="sidebar-list-icon" @click="emit('update:collapsed', !collapsed)">
      <Menu :size="22" :stroke-width="1.9" />
    </div>
    <div class="logo" @click="emit('open-view', 'home')">
      <LibraryBig :size="22" :stroke-width="1.8" />
    </div>
    <div :class="['side-menu-container-parent', collapsed ? 'collapsed-parent' : '']">
      <ul class="side-menu-container">
        <li
          v-for="tab in tabs"
          :key="tab.key"
          :class="['side-menu-item', activeView === tab.key ? 'active' : '']"
          @click="emit('open-view', tab.key)"
        >
          <div class="side-menu-selector">
            <div class="side-menu-icon">
              <Home v-if="tab.key === 'home'" :size="20" :stroke-width="1.9" />
              <Star v-else-if="tab.key === 'favorites'" :size="20" :stroke-width="1.9" />
              <StickyNote v-else-if="tab.key === 'notes'" :size="20" :stroke-width="1.9" />
              <Highlighter v-else-if="tab.key === 'highlights'" :size="20" :stroke-width="1.9" />
              <Bookmark v-else-if="tab.key === 'bookmarks'" :size="20" :stroke-width="1.9" />
              <Trash2 v-else-if="tab.key === 'trash'" :size="20" :stroke-width="1.9" />
            </div>
            <span>{{ tab.label }}</span>
          </div>
        </li>
      </ul>
      <div :class="['side-shelf-title-container', collapsed ? 'hidden' : '']">
        <div class="side-shelf-title">Shelf</div>
        <span
          :class="['side-shelf-title-icon', shelfCollapsed ? 'rotated' : '']"
          @click="emit('update:shelfCollapsed', !shelfCollapsed)"
        >
          <ChevronDown :size="14" :stroke-width="1.9" />
        </span>
      </div>
      <ul v-if="!shelfCollapsed && !collapsed" class="side-shelf-container">
        <li
          v-for="shelf in shelves"
          :key="shelf.id"
          :class="['side-menu-item', activeShelfTitle === shelf.name ? 'active' : '']"
          @click="emit('open-shelf', shelf)"
        >
          <div class="side-menu-selector">
            <div class="side-menu-icon"><LibraryBig :size="19" :stroke-width="1.6" /></div>
            <span class="sidebar-shelf-content">
              <span class="sidebar-shelf-name">{{ shelf.name }}</span>
              <span class="sidebar-shelf-count">{{ shelf.book_ids.length }}</span>
            </span>
          </div>
        </li>
      </ul>
      <div v-if="!collapsed" class="side-menu-selector sidebar-action-btn" style="cursor:pointer">
        <div class="side-menu-icon sidebar-action-icon"><Plus :size="11" :stroke-width="2" /></div>
        <span>New shelf</span>
      </div>
      <div v-if="!collapsed" class="side-menu-selector sidebar-action-btn" style="cursor:pointer">
        <div class="side-menu-icon sidebar-action-icon"><Pencil :size="13" :stroke-width="1.9" /></div>
        <span>Manage shelf</span>
      </div>
      <div v-if="!collapsed" class="side-shelf-title-container" style="margin-top:16px">
        <div class="side-shelf-title">Drives</div>
      </div>
      <div v-if="!collapsed" class="side-menu-selector" style="cursor:pointer" @click="emit('scan-now')">
        <div class="side-menu-icon">
          <CloudDownload :size="17" :stroke-width="1.8" :class="isScanning ? 'icon-rotate' : ''" />
        </div>
        <span>Scan drives</span>
      </div>
      <div v-if="!collapsed && totalCount > 0" class="side-menu-selector" style="cursor:pointer" @click="emit('clear-library')">
        <div class="side-menu-icon">
          <Trash2 :size="17" :stroke-width="1.8" />
        </div>
        <span>Clear all</span>
      </div>
    </div>
    <div class="side-menu-about">
      <div class="side-menu-selector" style="cursor:pointer" @click="emit('show-stats')">
        <div class="side-menu-icon"><BarChart3 :size="16" :stroke-width="1.9" /></div>
        <span v-if="!collapsed">Reading Stats</span>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.sidebar { width:190px; height:100%; position:absolute; left:0; top:0; z-index:10; transition:width .1s; }
.sidebar.collapsed { width:40px; }
.sidebar.collapsed .logo { display:none; }
.sidebar.collapsed .side-menu-item span { display:none; }
.sidebar.collapsed .side-menu-selector { justify-content:center; padding-left:0; }
.sidebar.collapsed .side-shelf-title-container,
.sidebar.collapsed .side-shelf-container,
.sidebar.collapsed .side-menu-about span { display:none; }
.sidebar.collapsed .collapsed-parent { width:70px !important; }
.sidebar.collapsed .side-menu-about .side-menu-selector { justify-content:center; }

.logo { position:absolute; top:18px; left:82px; cursor:pointer; color:var(--color-text-1); display:flex; align-items:center; }
.sidebar-list-icon { width:45px; height:50px; position:absolute; top:20px; left:10px; cursor:pointer; display:flex; justify-content:center; align-items:center; color:var(--color-text-2); }
.sidebar-list-icon:hover { border-radius:50%; background:var(--color-fill-2); }

.side-menu-container-parent { position:relative; top:85px; width:210px; height:calc(100% - 145px); overflow-x:hidden; overflow-y:auto; }
.side-menu-container-parent::-webkit-scrollbar { width:2px; }
.side-menu-container-parent::-webkit-scrollbar-thumb { display:none; }
.side-menu-container-parent:hover::-webkit-scrollbar-thumb { display:block; width:2px; }
.collapsed-parent { width:70px !important; }

.side-menu-container,.side-shelf-container { width:100%; padding-right:17px; box-sizing:content-box; position:relative; }
.side-menu-item { list-style:none; font-size:15px; width:100%; cursor:pointer; margin-left:-20px; margin-bottom:3px; }
.side-menu-selector { width:100%; height:39px; display:flex; align-items:center; font-weight:500; color:var(--color-text-2); border-radius:25px; padding-left:4px; }
.side-menu-selector:hover { background:var(--color-fill-2); }
.side-menu-item.active .side-menu-selector { background:var(--color-fill-2); color:rgb(var(--primary-6)); }
.side-menu-icon { font-size:22px; margin:0 12px 0 18px; display:flex; align-items:center; width:30px; color:var(--color-text-2); }
.active .side-menu-icon { color:rgb(var(--primary-6)); }

.sidebar-shelf-content { width:calc(100% - 70px); display:grid; grid-template-columns:minmax(0,1fr) auto; align-items:center; gap:8px; }
.sidebar-shelf-name { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; text-align:left; }
.sidebar-shelf-count { font-size:12px; opacity:.65; margin-right:20px; }
.side-shelf-title-container { width:70%; margin-left:10%; border-bottom:1px solid var(--color-border); margin-top:10px; margin-bottom:5px; display:flex; align-items:center; }
.side-shelf-title { font-size:16px; font-weight:600; margin:10px 0; display:inline-block; color:var(--color-text-1); }
.side-shelf-title-icon { margin-left:auto; cursor:pointer; transition:transform .1s; color:var(--color-text-2); }
.side-shelf-title-icon.rotated { transform:rotate(-90deg); }
.side-menu-about { position:absolute; bottom:0; left:0; width:100%; padding-bottom:8px; }
.side-menu-about .side-menu-selector { padding:0 8px; }

.sidebar-action-btn { height:34px !important; }
.sidebar-action-btn:hover { background:var(--color-fill-2); border-radius:25px; }
.sidebar-action-icon {
  border-radius:5px; background:rgba(128,128,128,0.08);
  padding:4px 0; width:24px; height:14px;
  margin-left:20px; margin-right:15px; display:flex; align-items:center; justify-content:center;
}

@keyframes rotate { from { transform:rotate(0deg) } to { transform:rotate(360deg) } }
.icon-rotate { animation:rotate 2s linear infinite; }
</style>
