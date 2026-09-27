import { Component } from '@angular/core';
import { RouterLink } from '@angular/router';
import { IS_DEMO } from '../demo/demo';

@Component({
  selector: 'app-about-nickles', imports: [RouterLink],
  template: `<h1>Nana-nickles: pocket-sized promises</h1>
    <section class="panel">
      <p>Anyone can package coins they own into a voucher, on screen or on paper. Only Nana can issue new money. A bearer voucher belongs, operationally, to whoever can present its secret first. A photograph or copy is another spendable copy. A signature prevents forgery, not copying or double spending.</p>
      <p>The showcase prototype is play money, valid only in this tab. Live Rust vouchers need HTTPS-only issuance/redemption, random secrets stored only as hashes, atomic single-use redemption, bounded liability tracking, and clear lost-token rules. They are not account login tokens.</p>
      @if (demo) { <p><a routerLink="/nickles">Try Nana-nickles</a></p> }
    </section>`,
})
export class AboutNicklesPage {
  readonly demo = IS_DEMO;
}
