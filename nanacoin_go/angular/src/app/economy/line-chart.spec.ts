// The chart's geometry, checked rather than eyeballed.
//
// A chart that is subtly wrong reads as authoritative, so the things that
// would go unnoticed in a screenshot - a line escaping its plot area, a label
// on the wrong end, a flat series dividing by zero - are asserted here.

import { ComponentRef } from '@angular/core';
import { ComponentFixture, TestBed } from '@angular/core/testing';

import { LineChart } from './line-chart';
import { Series } from './series';

const DAY = 86_400;

function series(name: string, values: [number, number][]): Series {
  return { name, points: values.map(([at, value]) => ({ at, value })) };
}

describe('LineChart', () => {
  let fixture: ComponentFixture<LineChart>;
  let ref: ComponentRef<LineChart>;

  beforeEach(() => {
    TestBed.configureTestingModule({ imports: [LineChart] });
    fixture = TestBed.createComponent(LineChart);
    ref = fixture.componentRef;
    ref.setInput('title', 'Money supply');
  });

  function render(all: Series[]): SVGSVGElement | null {
    ref.setInput('series', all);
    fixture.detectChanges();
    return fixture.nativeElement.querySelector('svg');
  }

  /** Every coordinate pair in a path, as numbers. */
  function points(path: string): { x: number; y: number }[] {
    return path
      .split(/[ML]/)
      .filter((s) => s.trim())
      .map((pair) => {
        const [x, y] = pair.split(',').map(Number);
        return { x, y };
      });
  }

  it('says so rather than drawing an empty plot', () => {
    render([series('Supply', [])]);
    expect(fixture.nativeElement.querySelector('svg')).toBeNull();
    expect(fixture.nativeElement.textContent).toContain('Nothing to plot yet');
  });

  it('keeps every point inside the plot area', () => {
    const svg = render([
      series('Supply', [
        [0, 0],
        [DAY, 500],
        [2 * DAY, 120],
      ]),
    ]);
    const path = svg!.querySelector('path.chart__line')!.getAttribute('d')!;

    // The padded plot box: left 52, right 720-16, top 16, bottom 240-28.
    for (const p of points(path)) {
      expect(p.x).toBeGreaterThanOrEqual(52);
      expect(p.x).toBeLessThanOrEqual(704);
      expect(p.y).toBeGreaterThanOrEqual(16);
      expect(p.y).toBeLessThanOrEqual(212);
    }
  });

  it('puts larger values higher up the chart', () => {
    const svg = render([
      series('Supply', [
        [0, 10],
        [DAY, 90],
      ]),
    ]);
    const [low, high] = points(svg!.querySelector('path.chart__line')!.getAttribute('d')!);
    // SVG y grows downwards, so the larger value must have the smaller y.
    expect(high.y).toBeLessThan(low.y);
  });

  it('survives a flat series instead of dividing by zero', () => {
    const svg = render([
      series('Supply', [
        [0, 50],
        [DAY, 50],
      ]),
    ]);
    const path = svg!.querySelector('path.chart__line')!.getAttribute('d')!;
    for (const p of points(path)) {
      expect(Number.isFinite(p.x)).toBe(true);
      expect(Number.isFinite(p.y)).toBe(true);
    }
  });

  it('includes zero on the axis, so a steady balance is not drawn as a cliff', () => {
    const svg = render([
      series('Balance', [
        [0, 100],
        [DAY, 104],
      ]),
    ]);
    // With a zero-based axis the 4-coin change must occupy a small slice of
    // the height, not the whole plot.
    const [a, b] = points(svg!.querySelector('path.chart__line')!.getAttribute('d')!);
    expect(Math.abs(a.y - b.y)).toBeLessThan(40);
  });

  it('spreads points out when they all share one timestamp', () => {
    // Unix seconds, so a burst of activity lands on the same second. Scaling
    // by elapsed time divides a zero span and stacks the whole chart on the
    // left edge as one vertical line - which is what it did before the
    // positional fallback existed.
    const at = 1_000_000;
    const svg = render([
      series('Supply', [
        [at, 0],
        [at, 100],
        [at, 250],
      ]),
    ]);
    const xs = points(svg!.querySelector('path.chart__line')!.getAttribute('d')!).map(
      (p) => p.x,
    );
    expect(new Set(xs).size).toBe(3);
    // Left edge to right edge of the plot area, in order.
    expect(xs[0]).toBe(52);
    expect(xs[2]).toBe(704);
    expect(xs[0]).toBeLessThan(xs[1]);
    expect(xs[1]).toBeLessThan(xs[2]);
  });

  it('draws a visible stub for a series with one point', () => {
    // A lone point has no segment to stroke, so the path would render nothing
    // and the series would silently vanish.
    const svg = render([series('GDP', [[0, 42]])]);
    const d = svg!.querySelector('path.chart__line')!.getAttribute('d')!;
    const pts = points(d);
    expect(pts.length).toBe(2);
    expect(pts[0].y).toBe(pts[1].y);
    expect(pts[1].x).toBeGreaterThan(pts[0].x);
  });

  it('labels the last value and marks it', () => {
    const svg = render([
      series('Supply', [
        [0, 10],
        [DAY, 880],
      ]),
    ]);
    expect(svg!.querySelector('text.chart__endlabel')!.textContent!.trim()).toBe('880');
    expect(svg!.querySelectorAll('circle.chart__marker').length).toBe(1);
  });

  it('shows no legend for one series and a legend for several', () => {
    render([series('Supply', [[0, 1]])]);
    expect(fixture.nativeElement.querySelector('.legend')).toBeNull();

    render([
      series('Sam', [
        [0, 1],
        [DAY, 2],
      ]),
      series('Ivy', [
        [0, 3],
        [DAY, 4],
      ]),
    ]);
    expect(fixture.nativeElement.querySelectorAll('.legend__item').length).toBe(2);
  });

  it('gives each series its own colour, in fixed order', () => {
    const svg = render([
      series('Sam', [
        [0, 1],
        [DAY, 2],
      ]),
      series('Ivy', [
        [0, 3],
        [DAY, 4],
      ]),
    ]);
    const strokes = [...svg!.querySelectorAll('path.chart__line')].map((p) =>
      p.getAttribute('stroke'),
    );
    expect(strokes[0]).not.toBe(strokes[1]);
    expect(new Set(strokes).size).toBe(2);
  });

  it('offers the numbers as a table, which is the relief for low-contrast hues', () => {
    render([
      series('Supply', [
        [0, 10],
        [DAY, 20],
      ]),
    ]);
    const rows = fixture.nativeElement.querySelectorAll('.chart__table tbody tr');
    expect(rows.length).toBe(2);
    expect(fixture.nativeElement.querySelector('.chart__table')!.textContent).toContain('20');
  });

  it('describes itself for a screen reader', () => {
    const svg = render([
      series('Supply', [
        [0, 10],
        [DAY, 880],
      ]),
    ]);
    const label = svg!.getAttribute('aria-label')!;
    expect(label).toContain('Money supply');
    expect(label).toContain('from 10 to 880');
  });
});
