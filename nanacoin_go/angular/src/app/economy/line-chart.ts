// One time-series line chart, drawn as inline SVG.
//
// No charting library. The smallest credible one is larger than this entire
// application, and all this needs is a polyline, an axis and a hover readout -
// which is a page of geometry, not a dependency.
//
// Everything here is presentation. The series arrive already computed by
// series.ts, so this file never decides what a number means.

import { Component, computed, input, signal } from '@angular/core';

import { Series } from './series';

/** Plot geometry. The viewBox is fixed and the SVG scales to its container. */
const W = 720;
const H = 240;
const PAD = { top: 16, right: 16, bottom: 28, left: 52 };

interface Line {
  name: string;
  color: string;
  path: string;
  /** The last point, where the direct label and end marker go. */
  end: { x: number; y: number; value: number } | null;
}

interface Readout {
  x: number;
  when: string;
  rows: { name: string; color: string; value: number }[];
}

@Component({
  selector: 'app-line-chart',
  template: `
    <figure class="chart">
      <figcaption class="chart__caption">
        <span class="chart__title">{{ title() }}</span>
        @if (subtitle()) {
          <span class="chart__subtitle">{{ subtitle() }}</span>
        }
      </figcaption>

      @if (isEmpty()) {
        <p class="muted small">Nothing to plot yet.</p>
      } @else {
        <!--
          A legend only for two or more series. With one, the caption above
          already names what is plotted and a single swatch would restate it.
        -->
        @if (series().length > 1) {
          <ul class="legend">
            @for (l of lines(); track l.name) {
              <li class="legend__item">
                <span class="legend__swatch" [style.background]="l.color"></span>
                {{ l.name }}
              </li>
            }
          </ul>
        }

        <svg
          class="chart__svg"
          [attr.viewBox]="'0 0 ' + W + ' ' + H"
          role="img"
          [attr.aria-label]="title() + '. ' + describe()"
          (pointerleave)="hover.set(null)"
          (pointermove)="onMove($event)"
        >
          <!-- Gridlines: hairline, solid, recessive. -->
          @for (t of yTicks(); track t.value) {
            <line
              class="chart__grid"
              [attr.x1]="PAD.left"
              [attr.x2]="W - PAD.right"
              [attr.y1]="t.y"
              [attr.y2]="t.y"
            />
            <text class="chart__tick" [attr.x]="PAD.left - 8" [attr.y]="t.y + 4">
              {{ t.label }}
            </text>
          }

          <!-- The x axis itself, and the two dates that bound it. -->
          <line
            class="chart__axis"
            [attr.x1]="PAD.left"
            [attr.x2]="W - PAD.right"
            [attr.y1]="H - PAD.bottom"
            [attr.y2]="H - PAD.bottom"
          />
          <text class="chart__tick chart__tick--start" [attr.x]="PAD.left" [attr.y]="H - 8">
            {{ startLabel() }}
          </text>
          <text class="chart__tick chart__tick--end" [attr.x]="W - PAD.right" [attr.y]="H - 8">
            {{ endLabel() }}
          </text>

          @for (l of lines(); track l.name) {
            <path class="chart__line" [attr.d]="l.path" [attr.stroke]="l.color" />
            @if (l.end) {
              <!--
                End marker: r=4 with a 2px surface ring, so overlapping series
                stay legible where they cross.
              -->
              <circle
                class="chart__marker"
                [attr.cx]="l.end.x"
                [attr.cy]="l.end.y"
                r="4"
                [attr.fill]="l.color"
              />
              <text class="chart__endlabel" [attr.x]="l.end.x - 8" [attr.y]="l.end.y - 8">
                {{ l.end.value }}
              </text>
            }
          }

          @if (hover(); as h) {
            <line
              class="chart__crosshair"
              [attr.x1]="h.x"
              [attr.x2]="h.x"
              [attr.y1]="PAD.top"
              [attr.y2]="H - PAD.bottom"
            />
          }
        </svg>

        @if (hover(); as h) {
          <p class="chart__readout">
            <strong>{{ h.when }}</strong>
            @for (r of h.rows; track r.name) {
              <span class="chart__readout-row">
                <span class="legend__swatch" [style.background]="r.color"></span>
                {{ r.name }}: {{ r.value }}
              </span>
            }
          </p>
        }

        <!--
          The table is the accessible equivalent and the relief the palette
          check requires, since some series sit below 3:1 against the surface.
          Collapsed so it does not crowd the chart.
        -->
        <details class="chart__table">
          <summary>See the numbers</summary>
          <table>
            <thead>
              <tr>
                <th scope="col">When</th>
                @for (l of lines(); track l.name) {
                  <th scope="col">{{ l.name }}</th>
                }
              </tr>
            </thead>
            <tbody>
              @for (row of tableRows(); track row.at) {
                <tr>
                  <th scope="row">{{ row.when }}</th>
                  @for (cell of row.values; track $index) {
                    <td>{{ cell }}</td>
                  }
                </tr>
              }
            </tbody>
          </table>
        </details>
      }
    </figure>
  `,
})
export class LineChart {
  readonly title = input.required<string>();
  readonly subtitle = input('');
  readonly series = input.required<Series[]>();

  /**
   * Whether the y axis must include zero.
   *
   * True for money: a balance chart whose axis starts at 40 makes a steady
   * account look like a cliff. False would only be right for a series where
   * the relative change is the story, which none of these are.
   */
  readonly zeroBased = input(true);

  protected readonly W = W;
  protected readonly H = H;
  protected readonly PAD = PAD;

  protected readonly hover = signal<Readout | null>(null);

  protected readonly isEmpty = computed(() =>
    this.series().every((s) => s.points.length === 0),
  );

  /** Every timestamp any series has a point at, ascending. */
  private readonly times = computed(() => {
    const all = new Set<number>();
    for (const s of this.series()) for (const p of s.points) all.add(p.at);
    return [...all].sort((a, b) => a - b);
  });

  private readonly bounds = computed(() => {
    const times = this.times();
    const values: number[] = [];
    for (const s of this.series()) for (const p of s.points) values.push(p.value);

    let min = values.length ? Math.min(...values) : 0;
    let max = values.length ? Math.max(...values) : 0;
    if (this.zeroBased()) {
      min = Math.min(min, 0);
      max = Math.max(max, 0);
    }
    // A flat series has no range to scale against, so give it one and let the
    // line sit in the middle rather than dividing by zero.
    if (min === max) {
      min -= 1;
      max += 1;
    }
    return {
      t0: times[0] ?? 0,
      t1: times[times.length - 1] ?? 1,
      min,
      max,
    };
  });

  protected readonly lines = computed<Line[]>(() =>
    this.series().map((s, i) => {
      const colour = SERIES_COLOURS[i % SERIES_COLOURS.length];
      const pts = s.points.map((p, j) => ({
        x: this.x(p.at, j, s.points.length),
        y: this.y(p.value),
        value: p.value,
      }));
      const last = pts[pts.length - 1];

      // A single point has no segment to stroke, so the path alone would draw
      // nothing at all. Give it a short horizontal stub centred on the point,
      // so one day's worth of data is visible rather than silently absent.
      const path =
        pts.length === 1
          ? `M${pts[0].x - 12},${pts[0].y} L${pts[0].x + 12},${pts[0].y}`
          : pts.map((p, j) => `${j === 0 ? 'M' : 'L'}${p.x},${p.y}`).join(' ');

      return {
        name: s.name,
        color: colour,
        path,
        end: last ? { x: last.x, y: last.y, value: last.value } : null,
      };
    }),
  );

  protected readonly yTicks = computed(() => {
    const { min, max } = this.bounds();
    const steps = 4;
    return Array.from({ length: steps + 1 }, (_, i) => {
      const value = min + ((max - min) * i) / steps;
      return { value, y: this.y(value), label: round(value) };
    });
  });

  protected readonly tableRows = computed(() =>
    this.times().map((at) => ({
      at,
      when: when(at),
      values: this.series().map((s) => {
        const hit = s.points.find((p) => p.at === at);
        return hit ? String(hit.value) : '';
      }),
    })),
  );

  protected readonly startLabel = computed(() => when(this.bounds().t0));
  protected readonly endLabel = computed(() => when(this.bounds().t1));

  /** A one-line summary for screen readers, since the shape is visual. */
  protected describe(): string {
    return this.series()
      .map((s) => {
        const pts = s.points;
        if (pts.length === 0) return `${s.name}: no data`;
        return `${s.name}: from ${pts[0].value} to ${pts[pts.length - 1].value}`;
      })
      .join('. ');
  }

  protected onMove(event: PointerEvent): void {
    const svg = event.currentTarget as SVGSVGElement;
    const rect = svg.getBoundingClientRect();
    if (rect.width === 0) return;

    // Pointer position in viewBox units, since the SVG scales to its box.
    const vx = ((event.clientX - rect.left) / rect.width) * W;
    const times = this.times();
    if (times.length === 0) return;

    // Nearest point in time, so the readout tracks data rather than pixels.
    let nearest = times[0];
    for (const t of times) {
      if (Math.abs(this.x(t) - vx) < Math.abs(this.x(nearest) - vx)) nearest = t;
    }

    const rows: Readout['rows'] = [];
    this.series().forEach((s, i) => {
      const hit = s.points.find((p) => p.at === nearest);
      if (hit) {
        rows.push({
          name: s.name,
          color: SERIES_COLOURS[i % SERIES_COLOURS.length],
          value: hit.value,
        });
      }
    });

    this.hover.set({ x: this.x(nearest), when: when(nearest), rows });
  }

  /**
   * The x position of a moment.
   *
   * Timestamps are unix seconds, so a burst of activity - a demo seeded in one
   * go, or a parent settling three chores at once - can put every point on the
   * same second. Scaling by elapsed time then divides a zero span and stacks
   * the whole chart on the left edge as one vertical line, which is what it
   * did before this fallback existed.
   *
   * So when time does not separate the points, their order does: they are
   * spread evenly across the axis. The shape stays honest - the sequence is
   * real - and the axis labels still name the dates, which are all the same
   * date and say so.
   */
  private x(at: number, index = 0, count = 1): number {
    const { t0, t1 } = this.bounds();
    const plot = W - PAD.left - PAD.right;
    const span = t1 - t0;

    if (span > 0) return PAD.left + ((at - t0) / span) * plot;

    // No elapsed time to scale against: fall back to the point's position in
    // its own series, which is the order the transactions were committed in.
    if (count <= 1) return PAD.left + plot / 2;
    return PAD.left + (index / (count - 1)) * plot;
  }

  private y(value: number): number {
    const { min, max } = this.bounds();
    const span = max - min || 1;
    return H - PAD.bottom - ((value - min) / span) * (H - PAD.top - PAD.bottom);
  }
}

/**
 * The categorical hues, assigned in fixed order and never cycled by rank, so a
 * person keeps their colour when the set of people on the chart changes.
 *
 * Validated for colour-blind separation against this app's own surfaces in
 * both light and dark mode. Some sit below 3:1 against the surface, which is
 * why every chart carries direct end labels and a table.
 */
const SERIES_COLOURS = [
  '#2a78d6',
  '#eb6834',
  '#1baf7a',
  '#eda100',
  '#e87ba4',
  '#008300',
];

function when(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
  });
}

function round(v: number): string {
  return Math.abs(v) >= 1000 ? `${Math.round(v / 100) / 10}k` : String(Math.round(v));
}
