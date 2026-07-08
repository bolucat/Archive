import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring, useVideoConfig } from "remotion";

// ============ EDITABLE CONSTANTS ============
const BRAND = {
  bg: "#0B0E1A",
  surface: "#141828",
  text: "#F1F5F9",
  accent: "#00E3D5",
  accent2: "#6366F1",
  accent3: "#A855F7",
  muted: "#94A3B8",
  border: "#1E293B",
};

const STATS = [
  { icon: "🌐", value: "官网", label: "xbyvideohub.com", color: BRAND.accent },
  { icon: "⭐", value: "GitHub", label: "开源项目", color: "#F59E0B" },
  { icon: "📄", value: "MIT", label: "开源协议", color: BRAND.accent3 },
];

const PLATFORMS = ["macOS", "Windows", "Linux", "iOS", "tvOS"];

const GITHUB_URL = "github.com/gaozhangmin/aliyunpan";
const CTA_TEXT = "⭐ Star on GitHub";
const APP_STORE_URL = "App Store 搜索 BoxPlayer";
// ============================================

// Brand icon (same as Scene1)
const BrandIcon: React.FC<{ size: number; color: string }> = ({ size, color }) => (
  <svg width={size} height={size} viewBox="0 0 1024 1024" fill="none">
    <path
      d="M242.778 183.941C275.923 150.55 328.657 147.042 366.562 174.106C387.575 190.771 403.014 213.262 406.044 239.193C406.488 243.372 406.611 247.512 406.627 251.711V253.513L406.634 256.302L406.633 265.519C406.635 267.749 406.637 269.977 406.64 272.206L406.645 290.562L406.654 308.333C406.663 321.943 406.667 335.553 406.667 349.163L406.672 380.701C406.673 383.729 406.674 386.756 406.675 389.784V390.916L406.676 392.065C406.682 413.235 406.686 434.405 406.686 455.575V457.993L406.685 470.126V477.432L406.69 515.328L406.706 553.223C406.721 581.618 406.73 610.013 406.729 638.408V649.758L406.744 685.723L406.75 703.807L406.747 721.89C406.743 728.418 406.743 734.946 406.751 741.473L406.757 750.444L406.75 759.415L406.755 765.891C406.832 797.546 400.759 823.895 378.394 847.336C358.316 866.713 334.263 874.975 306.73 874.648C284.759 874.114 263.35 865.076 247.11 850.34L246.34 849.634L243.546 847.117C234.671 838.744 228.612 829.245 223.157 818.506L222.074 816.342L221.095 814.425C215.397 802.313 215.055 789.155 215.089 775.944L215.112 770.28L215.096 763.42C215.082 757.171 215.086 750.921 215.093 744.672L215.09 735.584L215.06 706.775L215.061 687.053C215.066 675.546 215.066 664.038 215.06 652.53L215.057 647.891L215.054 640.901C215.044 619.251 215.038 597.601 215.046 575.951L215.047 573.48L215.055 553.618L215.057 514.885L215.034 476.152C215.008 447.113 214.994 418.073 215.001 389.034L215.003 379.738L215.004 377.426L215 360.181C214.997 354.433 214.992 348.685 214.986 342.937L214.974 323.318L214.988 303.7C214.992 300.365 214.994 297.03 214.994 293.695L214.983 283.689C214.976 280.633 214.973 277.577 214.975 274.521L214.993 265.353C214.995 264.251 214.995 263.148 214.994 262.046L214.984 258.738C214.857 229.894 222.854 205.367 242.778 183.941Z"
      fill={color}
    />
    <path
      d="M533.774 274.691C536.303 276.351 538.792 278.059 541.269 279.797C544.561 282.084 547.911 284.284 551.25 286.5C552.809 287.539 554.368 288.578 555.926 289.617C556.745 290.163 557.565 290.71 558.409 291.272C566.543 296.695 566.543 296.695 569.447 298.632C571.473 299.982 573.499 301.333 575.526 302.683C580.682 306.12 585.837 309.559 590.988 313.004C601.438 319.988 611.905 326.945 622.406 333.852C623.326 334.456 624.245 335.061 625.192 335.683C626.124 336.297 627.056 336.91 628.017 337.542C643.248 347.559 658.467 357.591 673.563 367.813C674.182 368.231 674.801 368.649 675.438 369.08C710.816 393.013 710.816 393.013 715.404 414.005C716.82 422.291 716.364 430.833 716.297 439.205C716.291 441.326 716.286 443.446 716.283 445.566C716.27 451.292 716.238 457.017 716.201 462.742C716.165 468.751 716.144 474.761 716.121 480.77C716.08  490.858 716.026 500.946 715.964 511.034C715.893 522.676 715.844 534.318 715.806 545.96C715.769 557.188 715.715 568.416 715.654 579.644C715.628 584.41 715.609 589.175 715.594 593.941C715.576 599.558 715.544 605.175 715.501 610.791C715.487 612.846 715.479 614.9 715.475 616.954C715.441 636.019 713.189 650.404 699.75 664.938C691.972 672.289 682.84 678.024 674 684C672.124 685.28 670.249 686.561 668.375 687.844C664.654 690.388 660.928 692.927 657.199 695.461"
      fill={color}
      opacity="0.85"
    />
    <path
      d="M809 448C812.191 449.826 815.203 451.867 818.223 453.961C819.5 454.829 819.5 454.829 820.803 455.715C823.562 457.593 826.313 459.484 829.063 461.375C831.945 463.344 834.83 465.309 837.715 467.275C839.657 468.598 841.598 469.922 843.538 471.248C849.596 475.382 855.685 479.463 861.813 483.492C862.566 483.989 863.319 484.485 864.095 484.997C866.926 486.861 869.756 488.725 872.597 490.573C897.717 506.924 897.717 506.924 902 522C903.233 532.723 901.37 541.299 895.485 550.324C889.225 558.177 880.259 563.476 872 569C871.018 569.66 871.018 569.66 870.016 570.333C866.92 572.41 863.816 574.474 860.707 576.531C856.985 579.001 853.302 581.525 849.625 584.063C843.041 588.603 836.444 593.123 829.835 597.627C828.237 598.716 826.64 599.807 825.044 600.899C820.094 604.269 815.126 607.614 810.113 610.891C809.272 611.443 808.431 611.995 807.564 612.563C800.93 616.28 794.457 616.893 787 616C779.803 613.805 773.892 609.482 770 603C766.828 595.866 766.56 589.135 766.58 581.419"
      fill={color}
      opacity="0.7"
    />
  </svg>
);

export const Scene9CTA: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Icon
  const iconScale = spring({ frame, fps, config: { damping: 80, stiffness: 200 } });

  // Title
  const titleOpacity = interpolate(frame, [10, 25], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // Stats
  const statStart = 25;
  const statStagger = 10;

  // URL
  const urlOpacity = interpolate(frame, [60, 75], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });
  const urlY = interpolate(frame, [60, 75], [20, 0], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // CTA badge
  const ctaStart = 75;
  const ctaScale = spring({ frame: frame - ctaStart, fps, config: { damping: 60, stiffness: 200 } });
  const ctaOpacity = interpolate(frame, [ctaStart, ctaStart + 12], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });
  const pulseScale = interpolate(Math.sin((frame - ctaStart) * 0.1), [-1, 1], [1, 1.05]);

  // Platform badges
  const platStart = 90;
  const platOpacity = interpolate(frame, [platStart, platStart + 15], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill
      style={{
        background: `radial-gradient(circle at 50% 40%, ${BRAND.surface} 0%, ${BRAND.bg} 70%)`,
        justifyContent: "center",
        alignItems: "center",
      }}
    >
      {/* Glow */}
      <div
        style={{
          position: "absolute",
          width: 800,
          height: 800,
          borderRadius: "50%",
          background: `radial-gradient(circle, ${BRAND.accent}15 0%, transparent 70%)`,
        }}
      />

      <div style={{ textAlign: "center", zIndex: 1, width: 1100 }}>
        {/* Brand Icon */}
        <div
          style={{
            margin: "0 auto 16px",
            transform: `scale(${iconScale})`,
            filter: `drop-shadow(0 8px 30px ${BRAND.accent}55)`,
          }}
        >
          <BrandIcon size={90} color={BRAND.accent} />
        </div>

        {/* Title */}
        <h1
          style={{
            fontSize: 48,
            fontWeight: 800,
            color: BRAND.text,
            opacity: titleOpacity,
            margin: "0 0 36px",
            textShadow: `0 0 30px ${BRAND.accent}33`,
          }}
        >
          开源 · 免费 · 跨平台
        </h1>

        {/* Stats */}
        <div
          style={{
            display: "flex",
            gap: 36,
            justifyContent: "center",
            marginBottom: 40,
          }}
        >
          {STATS.map((stat, i) => {
            const sOpacity = interpolate(
              frame,
              [statStart + i * statStagger, statStart + i * statStagger + 12],
              [0, 1],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            const sScale = spring({
              frame: frame - statStart - i * statStagger,
              fps,
              config: { damping: 80, stiffness: 200 },
            });
            return (
              <div
                key={i}
                style={{
                  opacity: sOpacity,
                  transform: `scale(${sScale})`,
                  textAlign: "center",
                }}
              >
                <div style={{ fontSize: 40, marginBottom: 6 }}>{stat.icon}</div>
                <div style={{ fontSize: 28, fontWeight: 800, color: stat.color, marginBottom: 2 }}>
                  {stat.value}
                </div>
                <div style={{ fontSize: 18, color: BRAND.muted, fontWeight: 500 }}>
                  {stat.label}
                </div>
              </div>
            );
          })}
        </div>

        {/* GitHub URL */}
        <div
          style={{
            opacity: urlOpacity,
            transform: `translateY(${urlY}px)`,
            marginBottom: 24,
          }}
        >
          <div
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 12,
              padding: "14px 32px",
              borderRadius: 12,
              background: BRAND.surface,
              border: `1px solid ${BRAND.border}`,
              fontFamily: "monospace",
              fontSize: 26,
              color: BRAND.accent,
            }}
          >
            <svg width="28" height="28" viewBox="0 0 24 24" fill={BRAND.text}>
              <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0 0 24 12c0-6.63-5.37-12-12-12z" />
            </svg>
            {GITHUB_URL}
          </div>
        </div>

        {/* CTA */}
        <div
          style={{
            opacity: ctaOpacity,
            transform: `scale(${ctaScale * pulseScale})`,
            marginBottom: 24,
          }}
        >
          <div
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 10,
              padding: "16px 48px",
              borderRadius: 30,
              background: `linear-gradient(135deg, ${BRAND.accent} 0%, ${BRAND.accent2} 100%)`,
              color: "#0B0E1A",
              fontSize: 28,
              fontWeight: 800,
              boxShadow: `0 10px 40px ${BRAND.accent}55`,
            }}
          >
            {CTA_TEXT}
          </div>
        </div>

        {/* Platforms */}
        <div
          style={{
            display: "flex",
            gap: 14,
            justifyContent: "center",
            opacity: platOpacity,
          }}
        >
          {PLATFORMS.map((p, i) => (
            <div
              key={i}
              style={{
                padding: "8px 22px",
                borderRadius: 16,
                background: BRAND.surface,
                border: `1px solid ${BRAND.border}`,
                color: BRAND.muted,
                fontSize: 18,
                fontWeight: 600,
              }}
            >
              {p}
            </div>
          ))}
        </div>
      </div>
    </AbsoluteFill>
  );
};
