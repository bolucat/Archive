<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue'
import { Flame } from 'lucide-vue-next'

const props = defineProps<{ apiBase: string; cachedItems?: {term:string;score:number}[] }>()
const emit = defineEmits<{ 'select': [term: string] }>()
const loading=ref(false);const items=ref<{term:string;score:number}[]>([]);const hasInitialized=ref(false);const cloudRef=ref<HTMLElement|null>(null);let tagCloudInstance:any=null

async function fetchHotSearches(){if(props.cachedItems?.length){items.value=props.cachedItems;hasInitialized.value=true;return}loading.value=true;try{const resp=await fetch(`${props.apiBase}/hot-searches?limit=25`);const data=await resp.json();if(data?.code===0&&Array.isArray(data?.data?.hotSearches)){items.value=(data.data.hotSearches as any[]).sort((a:any,b:any)=>(b.score||0)-(a.score||0)).slice(0,25)}}catch{};loading.value=false}
function destroyCloud(){if(tagCloudInstance){try{tagCloudInstance.destroy?.()}catch{};tagCloudInstance=null}}
async function initCloud(){if(items.value.length===0||!cloudRef.value)return;await nextTick();destroyCloud();try{const TagCloud=(await import('TagCloud')as any).default;tagCloudInstance=TagCloud(cloudRef.value,items.value.map(i=>i.term),{radius:150,maxSpeed:'slow',initSpeed:'slow',direction:135,keep:true,containerClass:'ph-tagcloud',itemClass:'ph-tagcloud-item'})}catch{}}
function onClick(e:MouseEvent){const el=(e.target as HTMLElement).closest('.ph-tagcloud-item') as HTMLElement;if(el?.innerText)emit('select',el.innerText.trim())}
onMounted(async()=>{await fetchHotSearches();hasInitialized.value=true;await nextTick();setTimeout(()=>initCloud(),300)})
onUnmounted(()=>destroyCloud())
</script>

<template>
  <div class="ph-hot">
    <div class="ph-hot-header"><Flame :size="16" :stroke-width="1.5" class="ph-hot-icon" /><span>搜索热度</span></div>
    <div v-if="loading&&!hasInitialized" class="ph-hot-loading"><span class="gs-spinner" />搜索热度加载中...</div>
    <div v-else-if="items.length===0" class="ph-hot-empty">暂无热搜数据</div>
    <div v-else-if="items.length>0" ref="cloudRef" class="ph-hot-cloud" @click="onClick" />
  </div>
</template>

<style scoped>
.ph-hot{background:transparent;border:none;border-radius:0;box-shadow:none;padding:12px 16px;min-height:260px;display:flex;flex-direction:column}
.ph-hot-header{display:flex;align-items:center;gap:6px;font-size:14px;font-weight:600;color:var(--color-text-2);margin-bottom:8px}
.ph-hot-icon{color:rgb(var(--orange-6))}
.ph-hot-loading,.ph-hot-empty{font-size:13px;color:var(--color-text-4);text-align:center;padding:40px 0;flex:1}
.ph-hot-cloud{flex:1;min-height:220px;display:flex;align-items:center;justify-content:center;cursor:pointer}
.ph-hot-list{display:flex;flex-wrap:wrap;gap:8px;justify-content:center;align-content:center}
.ph-hot-tag{padding:4px 12px;font-size:13px;color:var(--color-text-2);background:var(--color-fill-1);border:0;border-radius:16px;cursor:pointer;transition:background .15s,color .15s;line-height:22px}
.ph-hot-tag:hover{color:rgb(var(--primary-6));background:rgba(var(--primary-6),.1)}
:deep(.ph-tagcloud){height:240px!important}
:deep(.ph-tagcloud-item){color:var(--color-text-2)!important;font-weight:600!important;font-size:14px!important;cursor:pointer!important;user-select:none!important;will-change:transform!important}
:deep(.ph-tagcloud-item:hover){color:rgb(var(--primary-6))!important}
.gs-spinner{display:inline-block;width:12px;height:12px;border:2px solid var(--color-border-2);border-top-color:rgb(var(--primary-6));border-radius:50%;vertical-align:middle;margin-right:4px;animation:ph-spin .6s linear infinite}
@keyframes ph-spin{to{transform:rotate(360deg)}}
@media(max-width:720px){.ph-hot{padding:8px 12px;min-height:200px}:deep(.ph-tagcloud){height:180px!important}}
</style>
