<script setup lang="ts"></script>

<template>
  <div class="desktop-loading boxplayer-radio-loading">
    <div class="boxplayer-radio-loading-noise"></div>
    <div class="boxplayer-radio-loading-wordmark" aria-label="BoxPlayer Radio">
      <span class="boxplayer-loading-word-main">BoxPlayer</span>
      <span class="boxplayer-loading-word-sub">Radio</span>
    </div>
    <div class="boxplayer-radio-loading-line"></div>
    <div class="boxplayer-radio-loading-state">LOADING</div>
  </div>
</template>

<style>
.desktop-loading.boxplayer-radio-loading {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  overflow: hidden;
  color: #fff;
  background:
    linear-gradient(115deg, transparent 0%, rgba(255, 83, 103, 0.055) 24%, transparent 42%, rgba(244, 210, 138, 0.052) 62%, transparent 82%),
    repeating-linear-gradient(90deg, rgba(255, 255, 255, 0.03) 0 1px, transparent 1px 54px),
    repeating-linear-gradient(0deg, rgba(255, 255, 255, 0.02) 0 1px, transparent 1px 46px),
    radial-gradient(circle at 50% 48%, rgba(0, 245, 212, 0.16), transparent 30%),
    linear-gradient(180deg, #020606 0%, #050607 42%, #000 100%);
}

.boxplayer-radio-loading::after {
  position: absolute;
  inset: 0;
  pointer-events: none;
  content: '';
  background:
    linear-gradient(90deg, rgba(0, 0, 0, 0.82), transparent 21%, transparent 79%, rgba(0, 0, 0, 0.82)),
    linear-gradient(180deg, rgba(0, 0, 0, 0.68), transparent 32%, transparent 64%, rgba(0, 0, 0, 0.74));
}

.boxplayer-radio-loading-noise {
  position: absolute;
  inset: 0;
  opacity: 0.04;
  background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 180 180' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='.9' numOctaves='2' stitchTiles='stitch'/%3E%3CfeColorMatrix type='saturate' values='0'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='.55'/%3E%3C/svg%3E");
  background-size: 180px 180px;
  mix-blend-mode: screen;
}

.boxplayer-radio-loading-wordmark {
  position: relative;
  z-index: 2;
  width: min(940px, 82vw);
  height: clamp(70px, 12vw, 136px);
  font-size: clamp(38px, 7vw, 104px);
  font-weight: 760;
  letter-spacing: 0;
  text-shadow:
    0 0 28px rgba(0, 245, 212, 0.22),
    0 22px 78px rgba(0, 0, 0, 0.56);
  animation: boxplayerLoadingWord 1.7s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

.boxplayer-loading-word-main,
.boxplayer-loading-word-sub {
  position: absolute;
  top: 50%;
  left: 50%;
  white-space: nowrap;
  transform: translate(-50%, -50%);
}

.boxplayer-loading-word-main {
  color: #f8f8f2;
  animation: boxplayerLoadingMain 1.9s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

.boxplayer-loading-word-sub {
  color: transparent;
  background: linear-gradient(94deg, rgba(255, 255, 255, 0.06), #fff 26%, rgba(244, 210, 138, 0.98) 48%, rgba(122, 215, 194, 0.9) 68%, rgba(255, 255, 255, 0.82));
  background-clip: text;
  background-size: 300% 100%;
  -webkit-text-fill-color: transparent;
  -webkit-background-clip: text;
  animation: boxplayerLoadingSub 1.9s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

.boxplayer-radio-loading-line {
  position: absolute;
  top: calc(50% + 78px);
  left: 50%;
  z-index: 2;
  width: min(520px, 58vw);
  height: 2px;
  background: linear-gradient(90deg, transparent, rgba(0, 245, 212, 0.88), rgba(244, 210, 138, 0.78), transparent);
  box-shadow:
    0 0 18px rgba(244, 210, 138, 0.24),
    0 0 34px rgba(122, 215, 194, 0.1);
  transform: translateX(-50%) scaleX(0.2);
  transform-origin: center;
  animation: boxplayerLoadingLine 1.8s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

.boxplayer-radio-loading-state {
  position: absolute;
  top: calc(50% + 106px);
  left: 50%;
  z-index: 2;
  color: rgba(255, 255, 255, 0.62);
  font-size: 11px;
  font-weight: 780;
  letter-spacing: 0.24em;
  text-shadow:
    0 0 18px rgba(244, 210, 138, 0.24),
    0 0 34px rgba(122, 215, 194, 0.12);
  transform: translateX(-50%);
  animation: boxplayerLoadingState 1.8s ease-in-out infinite alternate;
}

@keyframes boxplayerLoadingWord {
  0% {
    opacity: 0;
    filter: blur(16px);
    transform: translateY(18px) scale(0.96);
  }
  32% {
    opacity: 1;
    filter: blur(0);
  }
  100% {
    opacity: 0.86;
    transform: translateY(0) scale(1);
  }
}

@keyframes boxplayerLoadingMain {
  0% {
    opacity: 0;
    clip-path: inset(48% 0 49% 0);
    transform: translate(calc(-50% - 10px), -42%) skewX(-10deg) scaleX(1.08);
  }
  46% {
    opacity: 1;
    clip-path: inset(0);
    transform: translate(-50%, -50%) skewX(0) scaleX(1);
  }
  100% {
    opacity: 1;
    transform: translate(calc(-50% - clamp(62px, 9vw, 128px)), -50%) scale(0.998);
  }
}

@keyframes boxplayerLoadingSub {
  0%,
  28% {
    opacity: 0;
    clip-path: inset(52% 0 44% 0);
    background-position: 0 0;
    transform: translate(calc(-50% + clamp(70px, 11vw, 130px)), -50%) skewX(9deg) scaleX(1.06);
  }
  58% {
    opacity: 0.9;
    clip-path: inset(20% 0 18% 0);
    background-position: 52% 0;
  }
  100% {
    opacity: 1;
    clip-path: inset(0);
    background-position: 100% 0;
    transform: translate(calc(-50% + clamp(126px, 17vw, 230px)), -50%) scale(1);
  }
}

@keyframes boxplayerLoadingLine {
  0% {
    opacity: 0;
    transform: translateX(-50%) scaleX(0.05);
  }
  100% {
    opacity: 1;
    transform: translateX(-50%) scaleX(1);
  }
}

@keyframes boxplayerLoadingState {
  0% {
    opacity: 0.42;
  }
  100% {
    opacity: 0.88;
  }
}
</style>
