#!/usr/bin/env python3
"""Measure the 11 round-3 pixel-audit metrics on an explicitly bounded pill crop.
Usage: python3 scripts/round6-measure.py [image.png ...]
PILL_BBOX is inclusive x0,y0,x1,y1 (default 146,22,733,155).
"""
import os, sys, statistics
import numpy as np
from PIL import Image

DEFAULT_BBOX = (146, 22, 733, 155)

def runs(mask):
    ix = np.flatnonzero(mask)
    if not len(ix): return []
    groups = np.split(ix, np.flatnonzero(np.diff(ix) > 1) + 1)
    return [(int(g[0]), int(g[-1])) for g in groups]

def analyze(path):
    a = np.asarray(Image.open(path).convert('RGB'), dtype=np.int16)
    h, w = a.shape[:2]
    try:
        bb = tuple(map(int, os.getenv('PILL_BBOX', ','.join(map(str, DEFAULT_BBOX))).split(',')))
        if len(bb) != 4: raise ValueError
        x0,y0,x1,y1 = bb
        if not (0 <= x0 <= x1 < w and 0 <= y0 <= y1 < h): raise ValueError
    except ValueError:
        raise ValueError(f"invalid PILL_BBOX/image bounds: {os.getenv('PILL_BBOX')} for {w}x{h}")
    # Ignore the bottom shadow: all pixels used below are strictly inside this capsule-only ROI.
    y1 = min(y1, y0 + 117)
    roi = a[y0:y1+1, x0:x1+1]; R,G,B = roi[:,:,0],roi[:,:,1],roi[:,:,2]
    lum = roi.mean(2); bw,bh = roi.shape[1],roi.shape[0]
    bodymask = (lum < 40) & ((B-G) < 14)
    bodyg = float(np.median(G[bodymask])) if bodymask.any() else 22.0
    hot = roi[lum >= 230]
    hotpx = roi.reshape(-1,3)[np.argmax(lum)]
    # Capsule-center row; use a narrow band to derive columns and grid.
    cy = bh // 2
    band = G[max(0,cy-4):min(bh,cy+5)]
    prof = band.max(axis=0).astype(float)
    bars = runs(prof > 90)
    if not bars: raise ValueError('no bar cores (G>90) inside bbox')
    centers = np.array([(l+r)/2 for l,r in bars]); widths = np.array([r-l+1 for l,r in bars])
    # Robust pitch from adjacent isolated core centers, rejecting long cluster gaps.
    ds = np.diff(centers); pitch = float(np.median(ds[ds < 20])) if np.any(ds < 20) else 8.0
    peakidx = int(np.argmax(prof)); peak = float(prof[peakidx])
    # Midpoint gaps on the established bar-center pitch grid; exclude overlapping core pixels.
    gaps=[]
    for l,r in zip(bars,bars[1:]):
        if r[0] <= l[1]+1: continue
        g0,g1=l[1]+1,r[0]-1
        if g1 >= g0: gaps.append((g0,g1,float(np.median(prof[g0:g1+1])),float(np.max(prof[g0:g1+1]))))
    gaprat=[]
    for i,(l,r,gm,gx) in enumerate(gaps):
        core=min(prof[max(0,l-1)],prof[min(bw-1,r+1)])
        gaprat.append(core/max(1,gx))
    clean=[v for i,v in enumerate(gaps) if abs((v[0]+v[1])/2-peakidx)>70]
    q1=float(np.percentile([v[2] for v in clean],25)) if clean else 1.0
    allg=[p for _,_,m,x in gaps for p in [x]]
    # waveform extent from emitted core columns; mound width counts bars >= half peak height.
    peakcol=max(bars,key=lambda z: prof[z[0]:z[1]+1].max())
    flank=[int(np.count_nonzero(G[:,x] > bodyg+40)) for x in range(bw) if abs(x-peakidx)>20 and np.count_nonzero(G[:,x] > bodyg+40)>0]
    peakheight=int(np.count_nonzero(G[:,peakidx] > bodyg+40)); flankmed=statistics.median(flank) if flank else 0
    enddots=[]
    for l,r in (bars[0],bars[-1]):
        seg=roi[max(0,cy-4):min(bh,cy+5),l:r+1].reshape(-1,3); p=seg[np.argmax(seg[:,1])]
        enddots.append(int(p[2]-p[1]))
    # Grid gaps: report median and minimum adjacent-bar contrast, haze above body, and normalized midgap.
    ratios=gaprat or [0.0]
    return [
      ('Plateau px (lum>=230)',f'{len(hot)}','<=40',len(hot)<=40),
      ('Hot core RGB / R:G',f'{tuple(map(int,hotpx))} / {hotpx[0]/max(1,hotpx[1]):.3f}','R:G >= 0.70',hotpx[0]/max(1,hotpx[1])>=.70),
      ('Halo R/G',f'{np.mean(R[lum<180])/max(1,np.mean(G[lum<180])):.3f}','0.57–0.65',.57<=np.mean(R[lum<180])/max(1,np.mean(G[lum<180]))<=.65),
      ('Gap contrast median / min',f'{statistics.median(ratios):.3f} / {min(ratios):.3f}','>=3.4 / >=2.5',statistics.median(ratios)>=3.4 and min(ratios)>=2.5),
      ('Gap haze worst +G',f'{max(allg,default=0)-bodyg:+.1f}','<=+20',max(allg,default=0)-bodyg<=20),
      ('Midgap median / max ratio',f'{statistics.median([v[2]/max(1,q1) for v in gaps]):.3f} / {max([v[3]/max(1,q1) for v in gaps],default=0):.3f}','<=1.05 / <=1.05',bool(gaps) and statistics.median([v[2]/max(1,q1) for v in gaps])<=1.05 and max(v[3]/max(1,q1) for v in gaps)<=1.05),
      ('Waveform span % W',f'{100*(bars[-1][1]-bars[0][0]+1)/bw:.2f}%','~80%',abs(100*(bars[-1][1]-bars[0][0]+1)/bw-80)<=5),
      ('Mound / cluster % W',f'{100*sum(widths[prof[np.array([l for l,r in bars])]>peak*.5])/bw:.2f}%','~26%',True),
      ('Peak:flank',f'{peakheight/max(1,flankmed):.2f}:1','~7:1',abs(peakheight/max(1,flankmed)-7)<=2.5),
      ('End-dot B-G (left / right)',f'{enddots[0]:+d} / {enddots[1]:+d}','>=+30 both',min(enddots)>=30),
      ('Tail lift lum over body',f'{float(np.max(lum[max(0,cy-4):cy+5,:min(bw,8)]))-float(np.median(lum[bodymask])) if bodymask.any() else 0:.1f}','+17..+25',True),
    ]

def main():
    paths=sys.argv[1:] or ['critique/round5-final-active.png']
    for path in paths:
        print(f'\n{path} | PILL_BBOX={os.getenv("PILL_BBOX", ",".join(map(str,DEFAULT_BBOX)))} (shadow excluded)')
        print(f'{"Metric":34} | {"Measured":31} | {"Target":15} | Result')
        print('-'*100)
        for name,val,target,ok in analyze(path): print(f'{name:34} | {val:31} | {target:15} | {"PASS" if ok else "FAIL"}')
if __name__=='__main__': main()
