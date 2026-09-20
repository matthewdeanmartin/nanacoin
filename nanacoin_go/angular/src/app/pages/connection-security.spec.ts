import { TestBed } from '@angular/core/testing';
import { signal } from '@angular/core';
import { ApiBase } from '../api/api-base';
import { NanacoinService } from '../api/nanacoin.service';
import { ConnectionSecurity } from './connection-security';

describe('household connection security', () => {
  afterEach(() => { vi.unstubAllGlobals(); TestBed.resetTestingModule(); });
  it('cannot enable secure mode from an HTTP page even after confirmation', async () => {
    vi.stubGlobal('location', { protocol: 'http:', href: 'http://nanacoin.local/' });
    const requireHttps = vi.fn();
    TestBed.configureTestingModule({ providers: [
      { provide: ApiBase, useValue: { current: signal('/api/v1') } },
      { provide: NanacoinService, useValue: { requireHttps } },
    ] });
    const component = TestBed.createComponent(ConnectionSecurity).componentInstance;
    component.ready = true;
    await component.enable();
    expect(requireHttps).not.toHaveBeenCalled();
  });
  it('requires confirmation and reloads the HTTPS site after successful revocation', async () => {
    const reload = vi.fn();
    vi.stubGlobal('location', { protocol: 'https:', href: 'https://nanacoin.local/', reload });
    const requireHttps = vi.fn().mockResolvedValue(undefined);
    TestBed.configureTestingModule({ providers: [
      { provide: ApiBase, useValue: { current: signal('/api/v1') } },
      { provide: NanacoinService, useValue: { requireHttps } },
    ] });
    const component = TestBed.createComponent(ConnectionSecurity).componentInstance;
    await component.enable();
    expect(requireHttps).not.toHaveBeenCalled();
    component.ready = true;
    await component.enable();
    expect(requireHttps).toHaveBeenCalledOnce();
    expect(reload).toHaveBeenCalledOnce();
  });
});
