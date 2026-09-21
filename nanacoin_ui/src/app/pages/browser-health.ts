import { Component, signal } from '@angular/core';

export function browserHealth() {
  const memory = (performance as Performance & { memory?: { usedJSHeapSize: number; jsHeapSizeLimit: number } }).memory;
  return {
    uptime: Math.floor(performance.now() / 1000),
    cores: navigator.hardwareConcurrency || null,
    online: navigator.onLine,
    secure: window.isSecureContext,
    heap: memory ? `${(memory.usedJSHeapSize / 1048576).toFixed(1)} MiB used / ${(memory.jsHeapSizeLimit / 1048576).toFixed(0)} MiB limit` : 'Not exposed by this browser',
    viewport: `${window.innerWidth} × ${window.innerHeight} CSS pixels`,
  };
}

@Component({
  selector: 'app-browser-health',
  template: `<h1>Board health</h1>
    <p>This static showcase runs in your browser tab, not on an ESP32. These are browser-reported readings, not pretend board measurements.</p>
    <section class="panel"><dl>
      <dt>Page lifetime</dt><dd>{{ data().uptime }} seconds</dd>
      <dt>Logical processors exposed to the browser</dt><dd>{{ data().cores ?? 'Unavailable' }}</dd>
      <dt>JavaScript heap (non-standard, approximate)</dt><dd>{{ data().heap }}</dd>
      <dt>Viewport</dt><dd>{{ data().viewport }}</dd>
      <dt>Browser says network available</dt><dd>{{ data().online ? 'Yes (not an Internet connectivity test)' : 'No' }}</dd>
      <dt>Secure browser context</dt><dd>{{ data().secure ? 'Yes' : 'No' }}</dd>
    </dl><button class="btn" title="Measure the browser's current memory and runtime information again" (click)="refresh()">Refresh readings</button></section>
    <p>No network requests are made for these readings. The browser does not expose board SRAM, PSRAM, flash wear, die temperature, Wi-Fi signal or whole-device power here. No invented zeroes stand in for those values.</p>`,
})
export class BrowserHealth {
  readonly data = signal(browserHealth());
  refresh(): void { this.data.set(browserHealth()); }
}
