<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { Film, RefreshCw } from 'lucide-vue-next'

const props = defineProps<{ apiBase: string }>()
const emit = defineEmits<{ 'select': [title: string] }>()
interface DoubanItem { id?: number; title: string; cover?: string; desc?: string }

const cats=[{id:'douban-top250',label:'Top250'},{id:'douban-movie',label:'新片榜'},{id:'douban-weekly',label:'口碑榜'},{id:'douban-us-box',label:'北美票房'}]
const activeCategory=ref(cats[0].id);const loading=ref(false);const loadingMore=ref(false)
const items=ref<DoubanItem[]>([]);const imgFailed=ref<Set<number>>(new Set())
const currentPage=ref(1);const hasMore=ref(false);const loadTrigger=ref<HTMLElement>()

async function fetchData(categoryId:string,page:number,append:boolean){
  const isFirst=page===1&&!append;if(isFirst)loading.value=true;else loadingMore.value=true
  try{const resp=await fetch(`${props.apiBase}/douban-hot?category=${categoryId}&page=${page}&limit=25`);const data=await resp.json()
    if(data?.code===0&&Array.isArray(data?.data?.items)){const ni:DoubanItem[]=data.data.items.map((item:any)=>({id:item.id,title:item.title||'',cover:item.cover||'',desc:item.desc||''}));if(append)items.value.push(...ni);else items.value=ni;hasMore.value=data.data.hasMore??(ni.length>=25)}
  }catch{}finally{loading.value=false;loadingMore.value=false}
}
function selectCategory(id:string){activeCategory.value=id;items.value=[];currentPage.value=1;hasMore.value=false;fetchData(id,1,false)}
function loadMore(){if(loadingMore.value||!hasMore.value)return;currentPage.value++;fetchData(activeCategory.value,currentPage.value,true)}
function onImgError(id:number|undefined){if(id!=null)imgFailed.value=new Set(imgFailed.value).add(id)}
function extractTerm(title:string):string{return title.replace(/^【[\d.]+】/,'').replace(/^#\d+\s*/,'').trim()}
function getCoverUrl(item:DoubanItem):string{return item.cover?`${props.apiBase}/img?url=${encodeURIComponent(item.cover)}`:''}

let observer:IntersectionObserver|null=null
onMounted(()=>{fetchData(activeCategory.value,1,false);setTimeout(()=>{observer=new IntersectionObserver(e=>{if(e[0]?.isIntersecting)loadMore()},{rootMargin:'200px',threshold:.01});if(loadTrigger.value)observer.observe(loadTrigger.value)},500)})
onUnmounted(()=>observer?.disconnect())
</script>

<template>
  <div class="ph-douban">
    <div class="ph-douban-header"><Film :size="16" :stroke-width="1.5" class="ph-douban-header-icon" /><span>豆瓣影视</span><button v-if="!loading&&items.length>0" class="ph-douban-refresh" type="button" @click="selectCategory(activeCategory)"><RefreshCw :size="13" :stroke-width="1.5" /></button></div>
    <div class="ph-douban-tabs"><button v-for="cat in cats" :key="cat.id" class="ph-douban-tab" :class="{active:activeCategory===cat.id}" type="button" @click="selectCategory(cat.id)">{{cat.label}}</button></div>
    <div v-if="loading" class="ph-douban-skeleton"><div v-for="i in 6" :key="i" class="ph-douban-skel-item"><div class="ph-douban-skel-cover"/><div class="ph-douban-skel-title"/></div></div>
    <div v-else class="ph-douban-grid">
      <div v-for="item in items" :key="item.id||item.title" class="ph-douban-card" @click="emit('select',extractTerm(item.title))">
        <div class="ph-douban-cover-wrap"><img v-if="item.cover&&!imgFailed.has(item.id!)" :src="getCoverUrl(item)" referrerpolicy="no-referrer" class="ph-douban-cover" loading="lazy" @error="onImgError(item.id)"/><div v-else class="ph-douban-cover-fb"><Film :size="22" :stroke-width="1.3" /></div></div>
        <div class="ph-douban-title">{{item.title}}</div><div v-if="item.desc" class="ph-douban-desc">{{item.desc}}</div>
      </div>
    </div>
    <div v-if="loadingMore" class="ph-douban-loading">加载更多...</div>
    <div ref="loadTrigger" v-show="hasMore" class="ph-douban-trigger"/>
  </div>
</template>

<style scoped>
.ph-douban{background:var(--color-bg-2);border:1px solid var(--color-border-2);border-radius:10px;padding:16px}
.ph-douban-header{display:flex;align-items:center;gap:6px;font-size:14px;font-weight:600;color:var(--color-text-2);margin-bottom:10px}
.ph-douban-header-icon{color:rgb(var(--green-6))}
.ph-douban-refresh{margin-left:auto;display:inline-flex;align-items:center;padding:2px 4px;color:var(--color-text-4);background:transparent;border:0;cursor:pointer;border-radius:4px}
.ph-douban-refresh:hover{color:var(--color-text-2);background:var(--color-fill-1)}
.ph-douban-tabs{display:flex;gap:6px;margin-bottom:14px}
.ph-douban-tab{padding:3px 12px;font-size:12px;font-weight:500;color:var(--color-text-3);background:var(--color-fill-1);border:0;border-radius:14px;cursor:pointer;line-height:20px}
.ph-douban-tab.active{color:#fff;background:rgb(var(--primary-6))}
.ph-douban-tab:hover:not(.active){color:var(--color-text-2);background:var(--color-fill-2)}
.ph-douban-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(100px,1fr));gap:10px}
.ph-douban-card{cursor:pointer;border-radius:8px;transition:transform .15s}
.ph-douban-card:hover{transform:translateY(-2px)}
.ph-douban-cover-wrap{width:100%;aspect-ratio:3/4;border-radius:6px;overflow:hidden;background:var(--color-fill-2)}
.ph-douban-cover{width:100%;height:100%;object-fit:cover;display:block}
.ph-douban-cover-fb{display:flex;align-items:center;justify-content:center;width:100%;height:100%;color:var(--color-text-4);background:var(--color-fill-1)}
.ph-douban-title{font-size:12px;font-weight:500;color:var(--color-text-2);margin-top:6px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;line-height:16px}
.ph-douban-desc{font-size:11px;color:var(--color-text-4);margin-top:2px;line-height:14px}
.ph-douban-skeleton{display:grid;grid-template-columns:repeat(auto-fill,minmax(100px,1fr));gap:10px}
.ph-douban-skel-item{border-radius:8px}
.ph-douban-skel-cover{width:100%;aspect-ratio:3/4;border-radius:6px;background:var(--color-fill-2);animation:ph-shimmer 1.5s infinite}
.ph-douban-skel-title{width:70%;height:14px;margin-top:6px;border-radius:4px;background:var(--color-fill-2);animation:ph-shimmer 1.5s infinite}
@keyframes ph-shimmer{0%,100%{opacity:1}50%{opacity:.4}}
.ph-douban-loading{text-align:center;padding:12px;font-size:12px;color:var(--color-text-4)}
.ph-douban-trigger{height:1px}
@media(max-width:720px){.ph-douban{padding:12px}.ph-douban-grid{grid-template-columns:repeat(auto-fill,minmax(80px,1fr));gap:8px}}
</style>
