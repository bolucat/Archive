<template>
  <span class="thinking-orb" aria-hidden="true">
    <i class="thinking-orb-core" />
    <i class="thinking-orb-satellite thinking-orb-satellite--one" />
    <i class="thinking-orb-satellite thinking-orb-satellite--two" />
  </span>
</template>

<style scoped>
.thinking-orb { position: relative; display: inline-block; width: 30px; height: 30px; flex: 0 0 30px; border-radius: 50%; background: radial-gradient(circle at 32% 28%, #d9ccff 0 4%, #8e74ff 22%, rgba(70, 224, 190, .72) 47%, #261c54 75%); box-shadow: 0 0 0 1px rgba(191, 177, 255, .55), 0 0 19px rgba(111, 97, 255, .5), inset -5px -7px 10px rgba(19, 11, 54, .56); overflow: visible; animation: thinking-orb-breathe 2.4s ease-in-out infinite; }
.thinking-orb::before { position: absolute; inset: 4px; content: ''; border: 1px solid rgba(232, 248, 255, .43); border-radius: inherit; transform: rotate(-28deg) scaleX(.68); }
.thinking-orb-core { position: absolute; top: 7px; left: 8px; width: 7px; height: 7px; border-radius: 50%; background: #effdff; box-shadow: 0 0 8px #e8ffff; opacity: .78; }
.thinking-orb-satellite { position: absolute; width: 5px; height: 5px; border-radius: 50%; background: #83f5d3; box-shadow: 0 0 8px #83f5d3; }
.thinking-orb-satellite--one { top: -2px; right: 4px; animation: thinking-orb-one 2.8s linear infinite; }
.thinking-orb-satellite--two { bottom: 0; left: -4px; width: 4px; height: 4px; background: #b7a0ff; box-shadow: 0 0 7px #b7a0ff; animation: thinking-orb-two 3.4s linear infinite; }
@keyframes thinking-orb-breathe { 50% { transform: scale(1.08); filter: saturate(1.25) brightness(1.1); } }
@keyframes thinking-orb-one { to { transform: rotate(360deg) translateX(5px) rotate(-360deg); } }
@keyframes thinking-orb-two { to { transform: rotate(-360deg) translateX(5px) rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .thinking-orb, .thinking-orb-satellite { animation: none; } }
</style>
