import { Component, inject, signal } from '@angular/core';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { SectionTabs } from '../ui/section-tabs';

@Component({
  selector:'app-specification', imports:[RouterLink,SectionTabs],
  templateUrl:'./specification.html',
  styles:`.spec { font-size:.95rem; line-height:1.6; max-width:52rem; } p, dl, ol { margin-block:.65rem; }
    .spec-head { border-bottom:1px solid var(--line); margin-bottom:1rem; } .spec-head p { color:var(--ink-soft); font-size:.85rem; }
    h3 { margin-top:1.6rem; } dt { font-weight:650; margin-top:.65rem; } dd { margin:0; } li { margin-block:.35rem; }
    .notebook-line { font-family:'Nana Hand','Segoe Script','Snell Roundhand',cursive; font-size:1.35rem; margin:.5rem 0; padding:.4rem .9rem;
      border-left:3px solid var(--accent); background:var(--surface); }
    .example { margin:1rem 0; padding:.75rem 1rem; background:var(--surface); border:1px solid var(--line); border-radius:var(--radius); }
    .example h4 { margin:0 0 .4rem; }
    table { border-collapse:collapse; width:100%; font-size:.9rem; } th, td { text-align:left; padding:.3rem .5rem; border-bottom:1px solid var(--line); vertical-align:top; }
    .num { text-align:right; font-variant-numeric:tabular-nums; white-space:nowrap; } tr.total td { font-weight:650; border-top:2px solid var(--line); }
    .tag { font-size:.75rem; }
    .spec-link { color:var(--accent); border:0; padding:0; background:none; font:inherit; text-decoration:underline; cursor:pointer; }`,
})
export class SpecificationPage {
  readonly tabs=[{id:'spec-ncs',label:'NCS 2026'},{id:'spec-nces',label:'NCES 2026 (Extended)'},{id:'spec-roadmap',label:'Roadmap'}];
  readonly tab=signal('spec-ncs');
  constructor(){inject(ActivatedRoute).queryParamMap.pipe(takeUntilDestroyed()).subscribe(params=>{
    const tab=params.get('tab');this.tab.set(this.tabs.some(t=>t.id===tab)?tab!:'spec-ncs');
  });}
}
