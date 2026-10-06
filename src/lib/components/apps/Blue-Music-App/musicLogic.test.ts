import { describe, it, expect } from 'vitest';
import {
  norm, compareTracks, sortTracks, groupAlbums, groupArtists, searchTracks, fmtTime, totalDuration,
  makeQueue, nextPos, prevPos, setShuffle, cycleRepeat, enqueueNext, enqueueEnd, removeAt, pruneQueue, currentPath, shuffled, emptyQueue,
  parseState, defaultState, parsePlaylists, addToPlaylist, removeFromPlaylist, resolvePlaylist, repeatToMpris, repeatFromMpris,
  type Track, type QueueState,
} from './musicLogic';

const tr = (o: Partial<Track> & { path: string }): Track => ({ title: o.path, artist: '', album: '', albumArtist: '', trackNo: 0, discNo: 0, year: 0, genre: '', durationSecs: 100, ...o });
// deterministic "random": a fixed cycle of values
const seq = (...v: number[]) => { let i = 0; return () => v[i++ % v.length]; };

describe('norm / search', () => {
  it('is accent- and case-insensitive, incl. Polish ł', () => {
    expect(norm('Żółć ŁÓDŹ Beyoncé')).toBe('zolc lodz beyonce');
  });
  it('requires every word, across title/artist/album/genre', () => {
    const lib = [tr({ path: 'a', title: 'Zażółć', artist: 'Łona', album: 'Biały', genre: 'Rap' }), tr({ path: 'b', title: 'Other', artist: 'Someone' })];
    expect(searchTracks(lib, 'zazolc lona').map((t) => t.path)).toEqual(['a']);
    expect(searchTracks(lib, 'rap bialy').map((t) => t.path)).toEqual(['a']);
    expect(searchTracks(lib, 'lona nothing')).toEqual([]);
    expect(searchTracks(lib, '   ')).toBe(lib);
  });
});

describe('sorting & grouping', () => {
  const lib = [
    tr({ path: '3', title: 'C', artist: 'Bee', album: 'Second', trackNo: 1, year: 2005 }),
    tr({ path: '2', title: 'B', artist: 'Aaa', album: 'First', trackNo: 2, year: 2001 }),
    tr({ path: '1', title: 'A', artist: 'Aaa', album: 'First', trackNo: 1, year: 2001 }),
    tr({ path: '9', title: 'Loose' }),                                   // no tags → last
    tr({ path: '4', title: 'D2', artist: 'Aaa', album: 'First', discNo: 2, trackNo: 1, year: 2001 }),
    tr({ path: '5', title: 'Track 10', artist: 'Aaa', album: 'First', trackNo: 10, year: 2001 }),
  ];
  it('orders by artist → album → disc → track number (numeric, not alphabetic), unknown last', () => {
    expect(sortTracks(lib).map((t) => t.path)).toEqual(['1', '2', '5', '4', '3', '9']);
  });
  it('compareTracks is a consistent total order', () => {
    for (const a of lib) for (const b of lib) expect(Math.sign(compareTracks(a, b)) + Math.sign(compareTracks(b, a))).toBe(0); // antisymmetric (avoids +0/-0)
  });
  it('groups albums by (album artist | album) and sums duration', () => {
    const albums = groupAlbums(lib);
    expect(albums.map((a) => `${a.artist}/${a.album}`)).toEqual(['Aaa/First', 'Bee/Second', '/']);
    expect(albums[0].tracks.map((t) => t.path)).toEqual(['1', '2', '5', '4']);
    expect(albums[0].duration).toBe(400);
  });
  it('files compilations under the album artist, not each track artist', () => {
    const comp = [tr({ path: 'x', artist: 'Guest 1', albumArtist: 'Various', album: 'Hits', trackNo: 1 }), tr({ path: 'y', artist: 'Guest 2', albumArtist: 'Various', album: 'Hits', trackNo: 2 })];
    expect(groupAlbums(comp)).toHaveLength(1);
    const artists = groupArtists(comp);
    expect(artists.map((a) => [a.name, a.trackCount, a.albums.length])).toEqual([['Various', 2, 1]]);
  });
  it('artists are sorted, unknown artist last, same album name by two artists stays separate', () => {
    const l = [tr({ path: '1', artist: 'Zed', album: 'Greatest' }), tr({ path: '2', artist: 'Amy', album: 'Greatest' }), tr({ path: '3' })];
    expect(groupArtists(l).map((a) => a.name)).toEqual(['Amy', 'Zed', '']);
    expect(groupAlbums(l)).toHaveLength(3);
  });
  it('same artist spelled with different case/accents is one artist', () => {
    const l = [tr({ path: '1', artist: 'Beyoncé', album: 'A' }), tr({ path: '2', artist: 'BEYONCE', album: 'B' })];
    expect(groupArtists(l)).toHaveLength(1);
  });
});

describe('formatting', () => {
  it('fmtTime', () => {
    expect(fmtTime(0)).toBe('0:00'); expect(fmtTime(65)).toBe('1:05'); expect(fmtTime(3725)).toBe('1:02:05');
    expect(fmtTime(NaN)).toBe('0:00'); expect(fmtTime(-4)).toBe('0:00'); expect(fmtTime(Infinity)).toBe('0:00');
  });
  it('totalDuration', () => { expect(totalDuration([tr({ path: 'a', durationSecs: 10 }), tr({ path: 'b', durationSecs: 5.5 })])).toBe(15.5); });
});

const P = ['a', 'b', 'c', 'd'];
describe('queue — building & navigation', () => {
  it('plays in order starting from the chosen track', () => {
    const q = makeQueue(P, 'c', false, 'off');
    expect(q.items).toEqual(P); expect(currentPath(q)).toBe('c');
  });
  it('falls back to the first track when the start is unknown or the list empty', () => {
    expect(currentPath(makeQueue(P, 'zzz', false, 'off'))).toBe('a');
    expect(makeQueue([], null, false, 'off')).toEqual(emptyQueue(false, 'off'));
  });
  it('shuffle keeps the chosen track first and is a permutation', () => {
    const q = makeQueue(P, 'c', true, 'off', seq(0.9, 0.1, 0.5));
    expect(q.items[0]).toBe('c'); expect([...q.items].sort()).toEqual(P); expect(q.pos).toBe(0); expect(q.original).toEqual(P);
  });
  it('shuffled() is a pure Fisher–Yates permutation', () => {
    const src = [1, 2, 3, 4, 5, 6]; const out = shuffled(src, seq(0.3, 0.8, 0.1, 0.6, 0.4));
    expect([...out].sort()).toEqual(src); expect(src).toEqual([1, 2, 3, 4, 5, 6]);
  });
  it('nextPos: advances, stops at the end (auto), wraps with repeat all, repeat one replays only on auto', () => {
    const at = (pos: number, repeat: QueueState['repeat']): QueueState => ({ ...makeQueue(P, 'a', false, repeat), pos });
    expect(nextPos(at(1, 'off'), true)).toBe(2);
    expect(nextPos(at(3, 'off'), true)).toBeNull();
    expect(nextPos(at(3, 'off'), false)).toBe(3);        // manual Next at the very end: stay
    expect(nextPos(at(3, 'all'), true)).toBe(0);
    expect(nextPos(at(3, 'all'), false)).toBe(0);
    expect(nextPos(at(1, 'one'), true)).toBe(1);
    expect(nextPos(at(1, 'one'), false)).toBe(2);
    expect(nextPos(emptyQueue(), true)).toBeNull();
  });
  it('prevPos: steps back, wraps only with repeat all', () => {
    const at = (pos: number, repeat: QueueState['repeat']): QueueState => ({ ...makeQueue(P, 'a', false, repeat), pos });
    expect(prevPos(at(2, 'off'))).toBe(1); expect(prevPos(at(0, 'off'))).toBe(0); expect(prevPos(at(0, 'all'))).toBe(3); expect(prevPos(emptyQueue())).toBe(-1);
  });
  it('cycleRepeat goes off → all → one → off', () => {
    expect(cycleRepeat('off')).toBe('all'); expect(cycleRepeat('all')).toBe('one'); expect(cycleRepeat('one')).toBe('off');
  });
});

describe('queue — shuffle toggle & editing', () => {
  it('toggling shuffle never changes the current track, and off restores the original order', () => {
    const q0 = { ...makeQueue(P, 'a', false, 'off'), pos: 2 };           // playing "c"
    const on = setShuffle(q0, true, seq(0.2, 0.7, 0.4));
    expect(currentPath(on)).toBe('c'); expect(on.pos).toBe(0); expect([...on.items].sort()).toEqual(P);
    const off = setShuffle(on, false);
    expect(off.items).toEqual(P); expect(currentPath(off)).toBe('c'); expect(off.pos).toBe(2);
    expect(setShuffle(off, false)).toBe(off);
  });
  it('enqueueNext inserts after the current track; enqueueEnd appends', () => {
    let q = makeQueue(P, 'b', false, 'off');
    q = enqueueNext(q, 'x'); expect(q.items).toEqual(['a', 'b', 'x', 'c', 'd']); expect(currentPath(q)).toBe('b');
    q = enqueueEnd(q, ['y', 'z']); expect(q.items.slice(-2)).toEqual(['y', 'z']);
    expect(currentPath(enqueueEnd(emptyQueue(), ['k']))).toBe('k');
  });
  it('enqueue while shuffled also updates the original order', () => {
    const q = enqueueEnd(makeQueue(P, 'a', true, 'off', seq(0.5)), ['n']);
    expect(q.original).toContain('n'); expect(q.items).toContain('n');
    expect(setShuffle(q, false).items).toEqual([...P, 'n']);
  });
  it('removeAt keeps following the playing track', () => {
    const q = { ...makeQueue(P, 'c', false, 'off') };               // pos 2
    expect(currentPath(removeAt(q, 0))).toBe('c');
    expect(currentPath(removeAt(q, 3))).toBe('c');
    const gone = removeAt(q, 2); expect(gone.items).toEqual(['a', 'b', 'd']); expect(currentPath(gone)).toBe('d');
    expect(removeAt(q, 99)).toBe(q);
    expect(removeAt(makeQueue(['only'], 'only', false, 'off'), 0).pos).toBe(-1);
  });
  it('pruneQueue drops missing files and keeps the current track when possible', () => {
    const q = makeQueue(P, 'c', false, 'all');
    const p = pruneQueue(q, (x) => x !== 'a' && x !== 'd');
    expect(p.items).toEqual(['b', 'c']); expect(currentPath(p)).toBe('c'); expect(p.repeat).toBe('all');
    expect(pruneQueue(q, () => false).pos).toBe(-1);
  });
});

describe('persisted state', () => {
  it('round-trips valid state', () => {
    const s = { ...defaultState(), folders: ['/a', '/b'], volume: 0.3, muted: true, shuffle: true, repeat: 'one' as const, queue: ['x', 'y'], pos: 1, positionSecs: 42 };
    expect(parseState(JSON.stringify(s))).toEqual(s);
  });
  it('survives garbage, wrong types and out-of-range numbers', () => {
    expect(parseState('nope')).toEqual(defaultState());
    expect(parseState('null')).toEqual(defaultState());
    const s = parseState(JSON.stringify({ folders: [], volume: 7, repeat: 'sideways', queue: ['a', 5, '', 'b'], pos: 99, positionSecs: -3, muted: 'yes' }));
    expect(s.folders).toEqual(['HOME/Music']); expect(s.volume).toBe(1); expect(s.repeat).toBe('off');
    expect(s.queue).toEqual(['a', 'b']); expect(s.pos).toBe(1); expect(s.positionSecs).toBe(0); expect(s.muted).toBe(false);
  });
  it('de-duplicates folders', () => {
    expect(parseState(JSON.stringify({ folders: ['/a', '/a', '/b'] })).folders).toEqual(['/a', '/b']);
  });
});

describe('playlists', () => {
  it('parses defensively and drops duplicates / broken entries', () => {
    const raw = JSON.stringify([{ id: 'p1', name: 'Mix', tracks: ['a', 3, 'b'] }, { id: 'p1', name: 'dup', tracks: [] }, { name: 'no id' }, null, { id: 'p2', name: 'X' }]);
    expect(parsePlaylists(raw)).toEqual([{ id: 'p1', name: 'Mix', tracks: ['a', 'b'] }, { id: 'p2', name: 'X', tracks: [] }]);
    expect(parsePlaylists('{{{')).toEqual([]); expect(parsePlaylists('{}')).toEqual([]);
  });
  it('add / remove / resolve', () => {
    let pl = { id: 'p', name: 'n', tracks: ['a'] };
    expect(addToPlaylist(pl, ['a', 'b']).tracks).toEqual(['a', 'b']);
    expect(addToPlaylist(pl, ['a'])).toBe(pl);
    expect(addToPlaylist(pl, ['a'], true).tracks).toEqual(['a', 'a']);
    pl = addToPlaylist(pl, ['b', 'ghost']);
    expect(removeFromPlaylist(pl, 0).tracks).toEqual(['b', 'ghost']);
    const lib = new Map([['a', tr({ path: 'a' })], ['b', tr({ path: 'b' })]]);
    expect(resolvePlaylist(pl, lib).map((t) => t.path)).toEqual(['a', 'b']);
  });
});

describe('MPRIS mapping', () => {
  it('repeat modes map both ways', () => {
    for (const r of ['off', 'all', 'one'] as const) expect(repeatFromMpris(repeatToMpris(r))).toBe(r);
    expect(repeatFromMpris('???')).toBe('off');
  });
});
