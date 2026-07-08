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
function getHost(url: string): string { try { return new URL(url.startsWith('http') ? url : `https://${url}`).host.replace(/^www\./, '') } catch { return url.split('/')[0] || '分享链接' } }

const grouped = computed(()=>{const g:{key:string;name:string;color:string;items:PanHubLink[];total:number}[]=[];for(const[k,info]of Object.entries(props.platformInfo)){const items=props.merged[k];if(!items||!items.length)continue;if(props.filterPlatform!=='all'&&k!==props.filterPlatform)continue;g.push({key:k,name:info.name,color:info.color,items:sortItems(items),total:items.length})}return g})
</script>

<template>
  <div v-for="group in grouped" :key="group.key" class="ph-group">
    <div class="ph-group-header">
      <div class="ph-group-title">
        <span class="ph-group-mark" :style="{ backgroundColor: group.color }"></span>
        <span class="ph-group-name">{{ group.name }}</span>
      </div>
      <span class="ph-group-count">{{ group.total }} 个资源</span>
    </div>
    <div class="ph-group-grid">
      <article v-for="(item, idx) in group.items" :key="idx" class="ph-card" :style="{ '--platform-color': group.color }">
        <a class="ph-card-link" :href="item.url" target="_blank" rel="noopener noreferrer nofollow" :title="item.url">
          <span class="ph-card-kicker">{{ getHost(item.url) }}</span>
          <span class="ph-card-title">{{ item.note || item.url }}</span>
          <ExternalLink :size="14" :stroke-width="1.7" class="ph-card-external" />
        </a>
        <div class="ph-card-footer">
          <div class="ph-card-meta">
            <span v-if="item.datetime" class="ph-meta-tag"><Calendar :size="12" :stroke-width="1.7" />{{ formatDate(item.datetime) }}</span>
            <span v-if="item.password" class="ph-meta-tag ph-meta-pwd"><Lock :size="12" :stroke-width="1.7" />{{ item.password }}</span>
          </div>
          <div class="ph-card-actions">
            <button v-if="canSave(item.url)" class="ph-action-btn ph-save-btn" type="button" title="保存到网盘" @click.stop="handleSave(item.url,item.password)"><Download :size="15" :stroke-width="1.7" /></button>
            <button class="ph-action-btn" :class="{ copied: copiedUrl === item.url }" type="button" :title="copiedUrl === item.url ? '已复制' : '复制链接'" @click="handleCopy(item.url)">
              <Check v-if="copiedUrl === item.url" :size="15" :stroke-width="2" />
              <Copy v-else :size="15" :stroke-width="1.7" />
            </button>
          </div>
        </div>
      </article>
    </div>
  </div>
</template>

<style scoped>
.ph-group{margin-bottom:26px}
.ph-group-header{display:flex;align-items:center;justify-content:space-between;gap:16px;margin-bottom:12px}
.ph-group-title{display:flex;align-items:center;gap:10px;min-width:0}
.ph-group-mark{width:10px;height:22px;border-radius:999px;box-shadow:0 0 18px rgba(0,0,0,.16)}
.ph-group-name{font-size:15px;font-weight:800;color:var(--color-text-1);line-height:22px}
.ph-group-count{flex-shrink:0;padding:4px 9px;font-size:12px;font-weight:700;color:var(--color-text-3);background:var(--color-fill-1);border:1px solid var(--color-border-2);border-radius:999px}
.ph-group-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(260px,1fr));gap:14px}
.ph-card{position:relative;display:flex;flex-direction:column;min-height:146px;padding:16px;background:linear-gradient(145deg,var(--color-bg-2),var(--color-fill-1));border:1px solid var(--color-border-2);border-radius:8px;overflow:hidden;transition:transform .16s,border-color .16s,box-shadow .16s,background .16s}
.ph-card::before{content:'';position:absolute;inset:0 0 auto;height:3px;background:var(--platform-color);opacity:.95}
.ph-card:hover{transform:translateY(-2px);border-color:var(--platform-color);box-shadow:0 16px 38px rgba(0,0,0,.14)}
.ph-card-link{position:relative;display:flex;flex:1;flex-direction:column;gap:8px;min-width:0;color:inherit;text-decoration:none}
.ph-card-kicker{display:block;width:max-content;max-width:100%;padding:3px 8px;font-size:11px;font-weight:800;line-height:16px;color:var(--platform-color);background:var(--color-fill-1);border:1px solid var(--color-border-2);border-radius:999px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.ph-card-title{display:-webkit-box;-webkit-box-orient:vertical;-webkit-line-clamp:3;min-height:60px;overflow:hidden;color:var(--color-text-1);font-size:14px;font-weight:650;line-height:20px}
.ph-card-external{position:absolute;right:0;top:0;color:var(--color-text-4);opacity:0;transition:opacity .15s,color .15s}
.ph-card:hover .ph-card-external{opacity:1;color:var(--platform-color)}
.ph-card-footer{display:flex;align-items:flex-end;justify-content:space-between;gap:12px;margin-top:14px}
.ph-card-meta{display:flex;align-items:center;flex-wrap:wrap;gap:7px;min-width:0}
.ph-meta-tag{display:inline-flex;align-items:center;gap:4px;min-width:0;padding:3px 7px;font-size:11px;font-weight:650;color:var(--color-text-4);line-height:16px;background:var(--color-fill-1);border:1px solid var(--color-border-1);border-radius:999px}
.ph-meta-pwd{color:rgb(var(--primary-6));background:rgba(var(--primary-6),.08);border-color:rgba(var(--primary-6),.18)}
.ph-card-actions{display:flex;align-items:center;gap:6px;flex-shrink:0}
.ph-action-btn{display:inline-flex;align-items:center;justify-content:center;width:30px;height:30px;padding:0;color:var(--color-text-3);background:var(--color-fill-1);border:1px solid var(--color-border-2);border-radius:7px;cursor:pointer;transition:color .15s,background .15s,border-color .15s,transform .15s}
.ph-action-btn:hover{transform:translateY(-1px);color:var(--color-text-1);background:var(--color-bg-2);border-color:var(--color-border-3)}
.ph-action-btn.copied{color:rgb(var(--green-6));background:rgba(var(--green-6),.1);border-color:rgba(var(--green-6),.25)}
.ph-save-btn{color:var(--platform-color)}

@media (max-width:720px){
  .ph-group-grid{grid-template-columns:1fr}
  .ph-card{min-height:132px}
}

:global(body[arco-theme='dark']) .ph-group-name{color:rgba(255,255,255,.92)}
:global(body[arco-theme='dark']) .ph-group-count{color:rgba(255,255,255,.56);background:rgba(255,255,255,.055);border-color:rgba(255,255,255,.075)}
:global(body[arco-theme='dark']) .ph-card{background:linear-gradient(145deg,rgba(255,255,255,.075),rgba(255,255,255,.035));border-color:rgba(255,255,255,.085);box-shadow:inset 0 1px 0 rgba(255,255,255,.035)}
:global(body[arco-theme='dark']) .ph-card:hover{border-color:var(--platform-color);box-shadow:0 18px 44px rgba(0,0,0,.28),inset 0 1px 0 rgba(255,255,255,.05)}
:global(body[arco-theme='dark']) .ph-card-title{color:rgba(255,255,255,.9)}
:global(body[arco-theme='dark']) .ph-card-kicker{background:rgba(255,255,255,.055);border-color:rgba(255,255,255,.075)}
:global(body[arco-theme='dark']) .ph-meta-tag{color:rgba(255,255,255,.52);background:rgba(255,255,255,.045);border-color:rgba(255,255,255,.065)}
:global(body[arco-theme='dark']) .ph-meta-pwd{color:#9ea9ff;background:rgba(105,121,255,.12);border-color:rgba(105,121,255,.22)}
:global(body[arco-theme='dark']) .ph-action-btn{color:rgba(255,255,255,.58);background:rgba(255,255,255,.055);border-color:rgba(255,255,255,.075)}
:global(body[arco-theme='dark']) .ph-action-btn:hover{color:#fff;background:rgba(255,255,255,.09);border-color:rgba(255,255,255,.14)}
:global(body[arco-theme='dark']) .ph-save-btn{color:var(--platform-color)}
</style>
