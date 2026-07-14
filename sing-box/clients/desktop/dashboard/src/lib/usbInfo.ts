const USB_CLASS_NAMES: Record<number, string> = {
  0x01: "Audio",
  0x02: "CDC Control",
  0x03: "HID",
  0x05: "Physical",
  0x06: "Image",
  0x07: "Printer",
  0x08: "Mass Storage",
  0x09: "Hub",
  0x0a: "CDC Data",
  0x0b: "Smart Card",
  0x0d: "Content Security",
  0x0e: "Video",
  0x0f: "Personal Healthcare",
  0x10: "Audio/Video",
  0x11: "Billboard",
  0x12: "USB-C Bridge",
  0xdc: "Diagnostic",
  0xe0: "Wireless",
  0xef: "Miscellaneous",
  0xfe: "Application Specific",
  0xff: "Vendor Specific",
};

const hex2 = (value: number) => `0x${value.toString(16).padStart(2, "0")}`;

export function usbClassTriplet(cls: number, sub: number, proto: number): string {
  const name = cls === 0 ? hex2(cls) : (USB_CLASS_NAMES[cls] ?? hex2(cls));
  return sub > 0 || proto > 0 ? `${name} · ${hex2(sub)} · ${hex2(proto)}` : name;
}

const USB_SPEED_LABELS: Record<number, string> = {
  1: "Low Speed",
  2: "Full Speed",
  3: "High Speed",
  4: "Wireless",
  5: "SuperSpeed",
  6: "SuperSpeed+",
};

export function usbSpeedLabel(code: number): string | undefined {
  return USB_SPEED_LABELS[code];
}

export function bcdToVersion(bcd: number): string {
  return `${(bcd >> 8) & 0xff}.${(bcd >> 4) & 0x0f}${bcd & 0x0f}`;
}
