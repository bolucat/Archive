import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring, useVideoConfig } from "remotion";

// ============ EDITABLE CONSTANTS ============
const BRAND = {
  bg: "#0B0E1A",
  surface: "#141828",
  text: "#F1F5F9",
  accent: "#00E3D5",      // Brand teal from icon.svg
  accent2: "#6366F1",     // Indigo
  accent3: "#A855F7",     // Purple
  muted: "#94A3B8",
  border: "#1E293B",
};

const PROJECT_NAME = "BoxPlayer";
const PROJECT_NAME_CN = "小白羊网盘";
const TAGLINE = "多网盘 · 媒体库 · 书籍库 · 音乐 · 下载 · AI";
// ============================================

// Brand icon path from icon.svg (simplified geometric shape)
const BrandIcon: React.FC<{ size: number; color: string }> = ({ size, color }) => (
  <svg width={size} height={size} viewBox="0 0 1024 1024" fill="none">
    <path
      d="M242.778 183.941C275.923 150.55 328.657 147.042 366.562 174.106C387.575 190.771 403.014 213.262 406.044 239.193C406.488 243.372 406.611 247.512 406.627 251.711V253.513L406.634 256.302L406.633 265.519C406.635 267.749 406.637 269.977 406.64 272.206L406.645 290.562L406.654 308.333C406.663 321.943 406.667 335.553 406.667 349.163L406.672 380.701C406.673 383.729 406.674 386.756 406.675 389.784V390.916L406.676 392.065C406.682 413.235 406.686 434.405 406.686 455.575V457.993L406.685 470.126V477.432L406.69 515.328L406.706 553.223C406.721 581.618 406.73 610.013 406.729 638.408V649.758L406.744 685.723L406.75 703.807L406.747 721.89C406.743 728.418 406.743 734.946 406.751 741.473L406.757 750.444L406.75 759.415L406.755 765.891C406.832 797.546 400.759 823.895 378.394 847.336C358.316 866.713 334.263 874.975 306.73 874.648C284.759 874.114 263.35 865.076 247.11 850.34L246.34 849.634L243.546 847.117C234.671 838.744 228.612 829.245 223.157 818.506L222.074 816.342L221.095 814.425C215.397 802.313 215.055 789.155 215.089 775.944L215.112 770.28L215.096 763.42C215.082 757.171 215.086 750.921 215.093 744.672L215.09 735.584L215.06 706.775L215.061 687.053C215.066 675.546 215.066 664.038 215.06 652.53L215.057 647.891L215.054 640.901C215.044 619.251 215.038 597.601 215.046 575.951L215.047 573.48L215.055 553.618L215.057 514.885L215.034 476.152C215.008 447.113 214.994 418.073 215.001 389.034L215.003 379.738L215.004 377.426L215 360.181C214.997 354.433 214.992 348.685 214.986 342.937L214.974 323.318L214.988 303.7C214.992 300.365 214.994 297.03 214.994 293.695L214.983 283.689C214.976 280.633 214.973 277.577 214.975 274.521L214.993 265.353C214.995 264.251 214.995 263.148 214.994 262.046L214.984 258.738C214.857 229.894 222.854 205.367 242.778 183.941Z"
      fill={color}
    />
    <path
      d="M533.774 274.691C536.303 276.351 538.792 278.059 541.269 279.797C544.561 282.084 547.911 284.284 551.25 286.5C552.809 287.539 554.368 288.578 555.926 289.617C556.745 290.163 557.565 290.71 558.409 291.272C566.543 296.695 566.543 296.695 569.447 298.632C571.473 299.982 573.499 301.333 575.526 302.683C580.682 306.12 585.837 309.559 590.988 313.004C601.438 319.988 611.905 326.945 622.406 333.852C623.326 334.456 624.245 335.061 625.192 335.683C626.124 336.297 627.056 336.91 628.017 337.542C643.248 347.559 658.467 357.591 673.563 367.813C674.182 368.231 674.801 368.649 675.438 369.08C710.816 393.013 710.816 393.013 715.404 414.005C716.82 422.291 716.364 430.833 716.297 439.205C716.291 441.326 716.286 443.446 716.283 445.566C716.27 451.292 716.238 457.017 716.201 462.742C716.165 468.751 716.144 474.761 716.121 480.77C716.08 490.858 716.026 500.946 715.964 511.034C715.893 522.676 715.844 534.318 715.806 545.96C715.769 557.188 715.715 568.416 715.654 579.644C715.628 584.41 715.609 589.175 715.594 593.941C715.576 599.558 715.544 605.175 715.501 610.791C715.487 612.846 715.479 614.9 715.475 616.954C715.441 636.019 713.189 650.404 699.75 664.938C691.972 672.289 682.84 678.024 674 684C672.124 685.28 670.249 686.561 668.375 687.844C664.654 690.388 660.928 692.927 657.199 695.461C652.641 698.56 648.099 701.682 643.563 704.813C637.955 708.68 632.303 712.476 626.628 716.243C621.531 719.631 616.484 723.087 611.446 726.561C596.621 736.781 581.711 746.878 566.807 756.983C559.794 761.738 552.79 766.505 545.813 771.313C545.184 771.743 544.556 772.174 543.908 772.618C541.398 774.34 538.9 776.072 536.43 777.852C526.331 784.995 516.377 787.471 504 786C493.43 783.292 485.439 777.239 479.625 768.063C474.776 759.796 473.555 751.955 473.596 742.492"
      fill={color}
      opacity="0.85"
    />
    <path
      d="M809 448C812.191 449.826 815.203 451.867 818.223 453.961C819.5 454.829 819.5 454.829 820.803 455.715C823.562 457.593 826.313 459.484 829.063 461.375C831.945 463.344 834.83 465.309 837.715 467.275C839.657 468.598 841.598 469.922 843.538 471.248C849.596 475.382 855.685 479.463 861.813 483.492C862.566 483.989 863.319 484.485 864.095 484.997C866.926 486.861 869.756 488.725 872.597 490.573C897.717 506.924 897.717 506.924 902 522C903.233 532.723 901.37 541.299 895.485 550.324C889.225 558.177 880.259 563.476 872 569C871.018 569.66 871.018 569.66 870.016 570.333C866.92 572.41 863.816 574.474 860.707 576.531C856.985 579.001 853.302 581.525 849.625 584.063C843.041 588.603 836.444 593.123 829.835 597.627C828.237 598.716 826.64 599.807 825.044 600.899C820.094 604.269 815.126 607.614 810.113 610.891C809.272 611.443 808.431 611.995 807.564 612.563C800.93 616.28 794.457 616.893 787 616C779.803 613.805 773.892 609.482 770 603C766.828 595.866 766.56 589.135 766.58 581.419C766.572 580.188 766.563 578.958 766.555 577.69C766.535 574.338 766.529 570.987 766.531 567.636C766.531 564.825 766.523 562.015 766.515 559.205C766.495 552.568 766.492 545.93 766.499 539.293C766.506 532.474 766.483 525.657 766.446 518.838C766.415 512.959 766.404 507.079 766.408 501.199C766.41 497.699 766.404 494.199 766.38 490.699C766.353 486.789 766.367 482.88 766.385 478.969C766.372 477.83 766.358 476.69 766.345 475.516C766.433 467.048 768.096 458.665 773.678 451.995C784.283 441.911 796.409 442.199 809 448Z"
      fill={color}
      opacity="0.7"
    />
  </svg>
);

export const Scene1Brand: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Icon entrance with spring
  const iconScale = spring({ frame, fps, config: { damping: 80, stiffness: 200 } });
  const iconOpacity = interpolate(frame, [0, 15], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // Glow pulse
  const glowPulse = interpolate(Math.sin(frame * 0.06), [-1, 1], [0.6, 1]);

  // Name slide up
  const nameY = interpolate(frame, [20, 45], [40, 0], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });
  const nameOpacity = interpolate(frame, [20, 45], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // Tagline
  const tagOpacity = interpolate(frame, [45, 65], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // Tech badges
  const badgeStart = 60;
  const BADGES = [
    { label: "Electron", color: "#47848F" },
    { label: "Vue 3", color: "#42B883" },
    { label: "TypeScript", color: "#3178C6" },
    { label: "MIT", color: "#A855F7" },
  ];

  // Exit
  const exitOpacity = interpolate(frame, [130, 150], [1, 0], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill
      style={{
        background: `radial-gradient(circle at 50% 50%, #1a1f3a 0%, ${BRAND.bg} 70%)`,
        justifyContent: "center",
        alignItems: "center",
        opacity: exitOpacity,
      }}
    >
      {/* Glow */}
      <div
        style={{
          position: "absolute",
          width: 500,
          height: 500,
          borderRadius: "50%",
          background: `radial-gradient(circle, ${BRAND.accent}${Math.floor(glowPulse * 40).toString(16).padStart(2, "0")} 0%, transparent 70%)`,
        }}
      />

      <div style={{ textAlign: "center", zIndex: 1 }}>
        {/* Brand Icon */}
        <div
          style={{
            margin: "0 auto 24px",
            opacity: iconOpacity,
            transform: `scale(${iconScale})`,
            filter: `drop-shadow(0 10px 40px ${BRAND.accent}66)`,
          }}
        >
          <BrandIcon size={140} color={BRAND.accent} />
        </div>

        {/* English Name */}
        <h1
          style={{
            fontSize: 80,
            fontWeight: 800,
            color: BRAND.text,
            margin: 0,
            opacity: nameOpacity,
            transform: `translateY(${nameY}px)`,
            letterSpacing: "2px",
          }}
        >
          {PROJECT_NAME}
        </h1>

        {/* Chinese Name */}
        <h2
          style={{
            fontSize: 36,
            fontWeight: 500,
            color: BRAND.accent,
            margin: "8px 0 16px",
            opacity: nameOpacity,
            transform: `translateY(${nameY}px)`,
          }}
        >
          {PROJECT_NAME_CN}
        </h2>

        {/* Tagline */}
        <p
          style={{
            fontSize: 28,
            color: BRAND.muted,
            margin: "0 0 32px",
            opacity: tagOpacity,
            letterSpacing: "2px",
          }}
        >
          {TAGLINE}
        </p>

        {/* Tech Badges */}
        <div style={{ display: "flex", gap: 16, justifyContent: "center" }}>
          {BADGES.map((badge, i) => {
            const bOpacity = interpolate(frame, [badgeStart + i * 8, badgeStart + i * 8 + 15], [0, 1], {
              extrapolateLeft: "clamp", extrapolateRight: "clamp",
            });
            const bY = interpolate(frame, [badgeStart + i * 8, badgeStart + i * 8 + 15], [15, 0], {
              extrapolateLeft: "clamp", extrapolateRight: "clamp",
            });
            return (
              <div
                key={i}
                style={{
                  padding: "8px 24px",
                  borderRadius: 20,
                  backgroundColor: `${badge.color}22`,
                  border: `1px solid ${badge.color}55`,
                  color: badge.color,
                  fontSize: 20,
                  fontWeight: 600,
                  opacity: bOpacity,
                  transform: `translateY(${bY}px)`,
                }}
              >
                {badge.label}
              </div>
            );
          })}
        </div>
      </div>
    </AbsoluteFill>
  );
};
