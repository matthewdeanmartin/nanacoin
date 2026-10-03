import { Injectable, inject } from '@angular/core';
import { NanacoinService } from '../api/nanacoin.service';
import { sha256Hex } from './art-media';

export interface StoredCopyright {
  bucket: string; key: string; hash: string; owner: string; title: string; license: string; revision: number;
}
type CloudSession = Awaited<ReturnType<NanacoinService['minicloudSession']>>;
@Injectable({ providedIn: 'root' })
export class MinicloudArt {
  private readonly bank = inject(NanacoinService);
  fileURL(url: string, c: {bucket: string; key: string}): string {
    return url + '/blobs/' + encodeURIComponent(c.bucket) + '/' + c.key.split('/').map(encodeURIComponent).join('/');
  }
  private async request(session: CloudSession, path: string, options: RequestInit = {}): Promise<any> {
    const response = await fetch(session.url + path, { ...options, signal: AbortSignal.timeout(15000),
      headers: { Authorization: 'Bearer ' + session.token, ...options.headers } });
    const result = await response.json();
    if (!response.ok) throw new Error(result.error ?? `Minicloud HTTP ${response.status}`);
    return result;
  }
  async list(search: string): Promise<{url: string; records: StoredCopyright[]}> {
    const session = await this.bank.minicloudSession();
    const result = await this.request(session, '/api/copyrights?q=' + encodeURIComponent(search));
    const files = await this.request(session, '/api/blobs');
    return {url: session.url, records: (result.copyrights as StoredCopyright[]).filter(c => c.owner === session.owner
      && files.blobs.some((b: {bucket: string; key: string; mime: string}) => b.bucket === c.bucket && b.key === c.key && b.mime.startsWith('image/')))};
  }
  async upload(file: File, title: string, license: string): Promise<{url: string; record: StoredCopyright}> {
    if (!file.type.startsWith('image/') || file.size > 256 * 1024) throw new Error('Choose an image up to 256 KiB.');
    const session = await this.bank.minicloudSession();
    if (!session.url.startsWith('https://')) throw new Error('Art editions require a minicloud HTTPS address. Configure HTTPS before uploading art.');
    const hash = await sha256Hex(await file.arrayBuffer());
    const key = session.namespace + '/' + hash;
    const path = '/api/copyrights/art/' + key;
    const records = await this.request(session, '/api/copyrights');
    const existing = (records.copyrights as StoredCopyright[]).find(c => c.bucket === 'art' && c.key === key);
    if (existing) {
      if (existing.hash !== hash || existing.owner !== session.owner) throw new Error('The stored copyright belongs to another owner.');
      return {url: session.url, record: existing};
    }
    await this.request(session, '/api/blobs/art/' + key, {method: 'PUT', headers: {'Content-Type': file.type}, body: file});
    const record: StoredCopyright = {bucket: 'art', key, hash, owner: session.owner, title, license, revision: 0};
    const result = await this.request(session, path, {method: 'PUT', headers: {'Content-Type': 'application/json'}, body: JSON.stringify(record)});
    return {url: session.url, record: result.copyright};
  }
}
