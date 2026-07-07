<script setup lang="ts">
import { ref, computed } from 'vue'
import { Copy, Check, Lock, Calendar, ExternalLink, Download } from 'lucide-vue-next'
import message from '../utils/message'
import { modalDaoRuShareLink } from '../utils/modal'

export interface PanHubLink { url: string; password: string; note: string; datetime: string; source?: string }

interface MergedLinks { [platform: string]: PanHubLink[] }

const props = defineProps<{ merged: MergedLinks; platformInfo: Record<string,{name:string;color:string}>; filterPlatform: string; sortType: string; initialVisible?: number }>()
const emit = defineEmits<{ copy: [url: string] }>()
const copiedUrl = ref('')
let copyTimer: ReturnType<typeof setTimeout>|null = null

const handleCopy = (url: string) => { navigator.clipboard.writeText(url).catch(()=>{}); copiedUrl.value = url; if(copyTimer)clearTimeout(copyTimer); copyTimer = setTimeout(()=>{copiedUrl.value=''},1500); emit('copy',url) }
function canSave(url: string): boolean { return /aliyundrive\.com\/s\/|alipan\.com\/s\/|quark\.cn\/s\/|123pan\.com\/s\//i.test(url) }
function parseSharePwd(url: string, password: string): string { if(password)return password; const m=url.match(/[?&#]pwd=([0-9a-zA-Z]+)/i)||url.match(/(?:提取码|密码)[^0-9a-zA-Z]{0,8}([0-9a-zA-Z]{4,8})/i); return m?.[1]||'' }
function handleSave(url: string, password: string) { if(canSave(url)){modalDaoRuShareLink(url,parseSharePwd(url,password))}else{message.info('暂不支持自动保存该网盘的分享链接，请手动复制链接后导入')} }
function formatDate(d: string): string { if(!d)return'';const dt=new Date(d);if(isNaN(dt.getTime()))return d;const days=Math.floor((Date.now()-dt.getTime())/86400000);if(days===0)return'今天';if(days===1)return'昨天';if(days<30)return`${days}天前`;if(days<365)return`${Math.floor(days/30)}个月前`;return dt.toLocaleDateString('zh-CN') }
function sortItems(items: PanHubLink[]): PanHubLink[]{ const s=[...items];if(props.sortType==='date-desc')s.sort((a,b)=>new Date(b.datetime||'').getTime()-new Date(a.datetime||'').getTime());else if(props.sortType==='date-asc')s.sort((a,b)=>new Date(a.datetime||'').getTime()-new Date(b.datetime||'').getTime());else if(props.sortType==='name-asc')s.sort((a,b)=>(a.note||'').localeCompare(b.note||''));else if(props.sortType==='name-desc')s.sort((a,b)=>(b.note||'').localeCompare(a.note||''));return s }

const grouped = computed(()=>{const g:{key:string;name:string;color:string;items:PanHubLink[];total:number}[]=[];for(const[k,info]of Object.entries(props.platformInfo)){const items=props.merged[k];if(!items||!items.length)continue;if(props.filterPlatform!=='all'&&k!==props.filterPlatform)continue;g.push({key:k,name:info.name,color:info.color,items:sortItems(items),total:items.length})}return g})
</script>

<template>
  <div v-for="group in grouped" :key="group.key" class="ph-group">
    <div class="ph-group-header"><div class="ph-group-badge" :style="{backgroundColor:group.color}">{{group.name}}</div><span class="ph-group-count">{{group.total}}个资源</span></div>
    <div class="ph-group-list">
      <div v-for="(item,idx) in group.items" :key="idx" class="ph-item">
        <div class="ph-item-main">
          <a class="ph-item-link" :href="item.url" target="_blank" rel="noopener noreferrer nofollow" :title="item.url"><span class="ph-item-note">{{item.note||item.url}}</span><ExternalLink :size="13" :stroke-width="1.5" class="ph-item-external" /></a>
          <div class="ph-item-meta"><span v-if="item.datetime" class="ph-meta-tag"><Calendar :size="11" :stroke-width="1.5" />{{formatDate(item.datetime)}}</span><span v-if="item.password" class="ph-meta-tag ph-meta-pwd"><Lock :size="11" :stroke-width="1.5" />{{item.password}}</span></div>
        </div>
        <button class="ph-copy-btn ph-save-btn" :class="{hidden:!canSave(item.url)}" type="button" title="保存到网盘" @click.stop="handleSave(item.url,item.password)"><Download :size="14" :stroke-width="1.5" /></button>
        <button class="ph-copy-btn" :class="{copied:copiedUrl===item.url}" type="button" :title="copiedUrl===item.url?'已复制':'复制链接'" @click="handleCopy(item.url)"><Check v-if="copiedUrl===item.url" :size="14" :stroke-width="2" /><Copy v-else :size="14" :stroke-width="1.5" /></button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ph-group{margin-bottom:20px;background:var(--color-bg-2);border:1px solid var(--color-border-2);border-radius:8px;overflow:hidden}
.ph-group-header{display:flex;align-items:center;gap:10px;padding:10px 16px;border-bottom:1px solid var(--color-border-2)}
.ph-group-badge{padding:2px 10px;font-size:12px;font-weight:600;color:#fff;border-radius:4px;line-height:20px}
.ph-group-count{font-size:12px;color:var(--color-text-4)}
.ph-group-list{max-height:400px;overflow-y:auto}
.ph-item{display:flex;align-items:center;gap:8px;padding:10px 16px;border-bottom:1px solid var(--color-border-1);transition:background .1s}
.ph-item:last-child{border-bottom:0}
.ph-item:hover{background:var(--color-fill-1)}
.ph-item-main{flex:1;min-width:0}
.ph-item-link{display:flex;align-items:center;gap:4px;color:var(--color-text-2);font-size:13px;line-height:20px;text-decoration:none;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.ph-item-link:hover{color:rgb(var(--primary-6))}
.ph-item-external{flex-shrink:0;color:var(--color-text-4);opacity:0;transition:opacity .15s}
.ph-item:hover .ph-item-external{opacity:1}
.ph-item-note{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.ph-item-meta{display:flex;align-items:center;gap:8px;margin-top:4px}
.ph-meta-tag{display:inline-flex;align-items:center;gap:3px;font-size:11px;color:var(--color-text-4);line-height:16px}
.ph-meta-pwd{color:rgb(var(--primary-6))}
.ph-copy-btn{display:inline-flex;align-items:center;justify-content:center;width:28px;height:28px;padding:0;flex-shrink:0;color:var(--color-text-3);background:transparent;border:0;border-radius:6px;cursor:pointer;transition:color .15s,background .15s}
.ph-copy-btn:hover{background:var(--color-fill-2);color:var(--color-text-2)}
.ph-copy-btn.copied{color:rgb(var(--green-6));background:rgba(var(--green-6),.1)}
.ph-save-btn{color:rgb(var(--primary-6));opacity:.5}
.ph-save-btn:hover{opacity:1;background:rgba(var(--primary-6),.1)}
.ph-save-btn.hidden{visibility:hidden}
</style>
