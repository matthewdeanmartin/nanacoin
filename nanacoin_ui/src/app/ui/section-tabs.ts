import { Component, input, model } from '@angular/core';

@Component({
  selector:'app-section-tabs',
  template:`<nav class="section-nav" role="tablist" [attr.aria-label]="label()">
    @for (tab of tabs(); track tab.id) {
      <button type="button" role="tab" [id]="prefix()+'-tab-'+tab.id" [attr.aria-controls]="tab.id"
        [attr.aria-selected]="selected()===tab.id" [attr.tabindex]="selected()===tab.id?0:-1"
        (click)="selected.set(tab.id)" (keydown)="key($event,tab.id)">{{tab.label}}</button>
    }
  </nav>`,
})
export class SectionTabs {
  readonly tabs=input.required<readonly {id:string;label:string}[]>();
  readonly selected=model.required<string>();
  readonly label=input.required<string>();
  readonly prefix=input.required<string>();
  key(event:KeyboardEvent,id:string):void {
    const tabs=this.tabs(),i=tabs.findIndex(t=>t.id===id);
    const next=event.key==='ArrowRight'?(i+1)%tabs.length:event.key==='ArrowLeft'?(i+tabs.length-1)%tabs.length:event.key==='Home'?0:event.key==='End'?tabs.length-1:-1;
    if(next<0)return;
    event.preventDefault();this.selected.set(tabs[next].id);
    document.getElementById(this.prefix()+'-tab-'+tabs[next].id)?.focus({preventScroll:true});
  }
}
