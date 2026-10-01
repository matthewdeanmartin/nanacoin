import { Injectable, computed, signal } from '@angular/core';
@Injectable({ providedIn: 'root' })
export class Activity {
  private readonly count = signal(0);
  readonly pending = computed(() => this.count() > 0);
  readonly failed = signal(false);
  private timer: ReturnType<typeof setTimeout> | undefined;
  begin(): void { this.count.update(n => n + 1); }
  end(): void { this.count.update(n => Math.max(0, n - 1)); }
  fail(): void {
    this.failed.set(true); clearTimeout(this.timer);
    this.timer = setTimeout(() => this.failed.set(false), 3000);
  }
}
