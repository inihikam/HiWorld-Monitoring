import { describe, it, expect } from 'vitest'
import { fmtBytes, fmtPercent, fmtLoad } from '../src/lib/format.js'

describe('fmtBytes (WO-AC-012)', () => {
  it('B → KB → MB → GB → TB', () => {
    expect(fmtBytes(0)).toBe('0 B')
    expect(fmtBytes(512)).toBe('512.0 B')
    expect(fmtBytes(1024)).toBe('1.0 KB')
    expect(fmtBytes(1536)).toBe('1.5 KB')
    expect(fmtBytes(1024 ** 2)).toBe('1.0 MB')
    expect(fmtBytes(1024 ** 3)).toBe('1.0 GB')
    expect(fmtBytes(1024 ** 4)).toBe('1.0 TB')
  })

  it('nilai ≥100 unit: tanpa desimal', () => {
    expect(fmtBytes(150 * 1024 ** 2)).toBe('150 MB')
  })

  it('null/NaN → —', () => {
    expect(fmtBytes(null)).toBe('—')
    expect(fmtBytes(NaN)).toBe('—')
  })

  it('sangat besar → cap di PB', () => {
    expect(fmtBytes(1024 ** 6)).toBe('1024 PB') // 1024 PB, tidak naik ke EB (cap)
    expect(fmtBytes(1024 ** 8)).toBe('1073741824 PB') // tetap unit PB
  })
})

describe('fmtPercent & fmtLoad', () => {
  it('fmtPercent 1 desimal', () => {
    expect(fmtPercent(42.56)).toBe('42.6%')
    expect(fmtPercent(0)).toBe('0.0%')
  })
  it('fmtLoad 2 desimal', () => {
    expect(fmtLoad(1.234)).toBe('1.23')
  })
  it('null → —', () => {
    expect(fmtPercent(null)).toBe('—')
    expect(fmtLoad(null)).toBe('—')
  })
})
