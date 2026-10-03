import { TestBed } from '@angular/core/testing';
import { vi } from 'vitest';
import { NanacoinService } from '../api/nanacoin.service';
import { MinicloudArt, StoredCopyright } from './minicloud-art';
import { sha256Hex } from './art-media';

describe('minicloud art registration', () => {
  const session = {url: 'https://cloud.local', token: 'delegation', owner: 'alice@bank', namespace: 'namespace', expires_at: 2000000000};
  let cloud: MinicloudArt;
  const bank = {minicloudSession: async () => session};
  beforeEach(() => {
    TestBed.configureTestingModule({providers: [MinicloudArt, {provide: NanacoinService, useValue: bank}]});
    cloud = TestBed.inject(MinicloudArt);
  });
  afterEach(() => vi.unstubAllGlobals());
  function file(): File {
    const f = new File(['picture'], 'moon.png', {type: 'image/png'});
    Object.defineProperty(f, 'arrayBuffer', {value: async () => new TextEncoder().encode('picture').buffer});
    return f;
  }
  it('does not overwrite a registered file on retry', async () => {
    const image = file();
    const hash = await sha256Hex(await image.arrayBuffer());
    const record: StoredCopyright = {bucket: 'art', key: 'namespace/' + hash, hash, owner: session.owner, title: 'Moon', license: 'Display', revision: 4};
    const requests: RequestInit[] = [];
    vi.stubGlobal('fetch', async (_url: string, options: RequestInit) => {
      requests.push(options); return new Response(JSON.stringify({copyrights: [record]}));
    });
    const stored = await cloud.upload(image, 'Moon', 'Display');
    expect(stored.record).toEqual(record);
    expect(requests.length).toBe(1);
    expect(requests[0].method).toBeUndefined();
    expect((requests[0].headers as Record<string,string>)['Authorization']).toBe('Bearer delegation');
  });
  it('never creates a copyright record after a failed file upload', async () => {
    const methods: string[] = [];
    vi.stubGlobal('fetch', async (_url: string, options: RequestInit) => {
      methods.push(options.method ?? 'GET');
      return methods.length === 1 ? new Response(JSON.stringify({copyrights: []}))
        : new Response(JSON.stringify({error: 'Storage full'}), {status: 413});
    });
    await expect(cloud.upload(file(), 'Moon', 'Display')).rejects.toThrow('Storage full');
    expect(methods).toEqual(['GET', 'PUT']);
  });
  it('lists only the signed-in copyright owner’s image files', async () => {
    const c = {bucket: 'art', key: 'mine', owner: session.owner};
    let n = 0;
    vi.stubGlobal('fetch', async () => new Response(JSON.stringify(++n === 1
      ? {copyrights: [c, {...c, key: 'other', owner: 'bob@bank'}, {...c, key: 'text'}]}
      : {blobs: [{bucket:'art', key:'mine',mime:'image/png'},{bucket:'art',key:'other',mime:'image/png'},{bucket:'art',key:'text',mime:'text/plain'}]})));
    expect((await cloud.list('')).records).toEqual([c]);
  });
});
