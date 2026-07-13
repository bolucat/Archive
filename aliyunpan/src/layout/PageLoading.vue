<script setup lang="ts"></script>

<template>
  <div class="desktop-loading boxplayer-splash-loading">
    <div class="boxplayer-splash-noise"></div>
    <div class="boxplayer-splash-wordmark" aria-label="BoxPlayer">
      <span class="boxplayer-splash-word-main">Box</span>
      <span class="boxplayer-splash-word-accent">Player</span>
    </div>
    <div class="boxplayer-splash-line"></div>
    <div class="boxplayer-splash-state">LOADING</div>
  </div>
</template>

<style>
.desktop-loading.boxplayer-splash-loading {
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

.boxplayer-splash-loading::after {
  position: absolute;
  inset: 0;
  pointer-events: none;
  content: '';
  background:
    linear-gradient(90deg, rgba(0, 0, 0, 0.82), transparent 21%, transparent 79%, rgba(0, 0, 0, 0.82)),
    linear-gradient(180deg, rgba(0, 0, 0, 0.68), transparent 32%, transparent 64%, rgba(0, 0, 0, 0.74));
}

.boxplayer-splash-noise {
  position: absolute;
  inset: 0;
  opacity: 0.04;
  background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 180 180' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='.9' numOctaves='2' stitchTiles='stitch'/%3E%3CfeColorMatrix type='saturate' values='0'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='.55'/%3E%3C/svg%3E");
  background-size: 180px 180px;
  mix-blend-mode: screen;
}

.boxplayer-splash-wordmark {
  position: relative;
  z-index: 2;
  display: inline-flex;
  align-items: baseline;
  justify-content: center;
  width: min(760px, 84vw);
  height: clamp(72px, 11vw, 132px);
  font-family: 'Avenir Next', 'Inter', 'SF Pro Display', 'Segoe UI', sans-serif;
  font-size: clamp(42px, 7.2vw, 106px);
  font-weight: 850;
  letter-spacing: 0;
  font-variant-ligatures: none;
  text-shadow:
    0 0 28px rgba(0, 245, 212, 0.22),
    0 22px 78px rgba(0, 0, 0, 0.56);
  animation: boxplayerSplashWord 1.7s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

.boxplayer-splash-word-main,
.boxplayer-splash-word-accent {
  position: relative;
  display: inline-block;
  white-space: nowrap;
}

.boxplayer-splash-word-main {
  color: #f8f8f2;
  animation: boxplayerSplashMain 1.8s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

.boxplayer-splash-word-accent {
  margin-left: 0.04em;
  color: transparent;
  background: linear-gradient(94deg, #fff 4%, rgba(244, 210, 138, 0.98) 36%, rgba(122, 215, 194, 0.94) 70%, rgba(255, 255, 255, 0.88));
  background-clip: text;
  background-size: 300% 100%;
  -webkit-text-fill-color: transparent;
  -webkit-background-clip: text;
  animation: boxplayerSplashAccent 1.9s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

.boxplayer-splash-wordmark::before,
.boxplayer-splash-wordmark::after {
  position: absolute;
  left: 50%;
  z-index: -1;
  width: min(560px, 62vw);
  height: 34%;
  pointer-events: none;
  content: '';
  border-radius: 999px;
  transform: translateX(-50%);
}

.boxplayer-splash-wordmark::before {
  top: 18%;
  background: linear-gradient(90deg, transparent, rgba(0, 245, 212, 0.13), transparent);
  filter: blur(14px);
}

.boxplayer-splash-wordmark::after {
  bottom: 18%;
  background: linear-gradient(90deg, transparent, rgba(244, 210, 138, 0.14), transparent);
  filter: blur(18px);
}

.boxplayer-splash-line {
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
  animation: boxplayerSplashLine 1.8s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

.boxplayer-splash-state {
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
  animation: boxplayerSplashState 1.8s ease-in-out infinite alternate;
}

@keyframes boxplayerSplashWord {
  0% {
    opacity: 0.48;
    filter: blur(10px);
    transform: translateY(10px) scale(0.98);
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

@keyframes boxplayerSplashMain {
  0% {
    opacity: 0.62;
    clip-path: inset(16% 0 18% 0);
    transform: translateX(-10px) skewX(-5deg) scaleX(1.04);
  }
  46% {
    opacity: 1;
    clip-path: inset(0);
    transform: translateX(0) skewX(0) scaleX(1);
  }
  100% {
    opacity: 1;
    transform: translateX(0) scale(0.998);
  }
}

@keyframes boxplayerSplashAccent {
  0%,
  28% {
    opacity: 0.42;
    clip-path: inset(18% 0 20% 0);
    background-position: 0 0;
    transform: translateX(12px) skewX(5deg) scaleX(1.03);
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
    transform: translateX(0) scale(1);
  }
}

@keyframes boxplayerSplashLine {
  0% {
    opacity: 0;
    transform: translateX(-50%) scaleX(0.05);
  }
  100% {
    opacity: 1;
    transform: translateX(-50%) scaleX(1);
  }
}

@keyframes boxplayerSplashState {
  0% {
    opacity: 0.42;
  }
  100% {
    opacity: 0.88;
  }
}
</style>
