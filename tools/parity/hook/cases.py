# SPDX-License-Identifier: GPL-3.0-or-later
"""Deterministic hook events and timer/feedback boundaries, never OS input."""
import json
from pathlib import Path
import random
import sys


def typed(text, time=10000, interval=120, **fields):
    steps = []
    for letter in text:
        vk = {" ": 32, "\n": 13, "\b": 8}.get(letter, ord(letter.upper()))
        steps += press(vk, time, **fields)
        time += interval
    return steps


def press(vk, time, **fields):
    event = dict(op="key", vk=vk, scan=vk + 7, extended=vk % 2 == 0, **fields)
    return [dict(event, time=time), dict(event, time=time+40, isUp=True)]


def drain():
    return [dict(op="take"), dict(op="take"), dict(op="snapshot")]


def fixtures():
    cases = []
    def add(name, steps, **settings):
        cases.append(dict(name=name, settings=settings, steps=steps))

    for word in ["konnichiwa", "hello", "localhost", "ka", "hima", "tabemo", "zanzok", "kyou", "nihongo",
                 "gakkou", "fucarete", "shitsumon", "github", "as", "to", "x", "unknown"]:
        add("type:"+word, typed(word)+drain()+press(32,15000)+drain())
    for level in ["Aggressive", "Conservative", "Manual"]:
        for word in ["konn", "hello", "ka", "hima", "tabemo", "github"]:
            add(f"level:{level}:{word}", typed(word)+drain()+press(13,11500), DetectionLevel=level)
    for style in ["Kana", "Both"]:
        for word in ["byiaf", "ka", "hello", "q", "1", ";", "@"]:
            add(f"style:{style}:{word}", typed(word)+drain()+press(32,11500), InputStyle=style)
    for delay in [699,700,701]:
        add(f"idle:{delay}", typed("ka")+[dict(op="timer",time=10120+delay)]+drain())
    for delay in [2499,2500,2501]:
        add(f"max-hold:{delay}", typed("ka",interval=400)+[dict(op="timer",time=10000+delay)]+drain(),IdleFlushMs=3000)
    for word in ["l", "hel"]:
        for delay in [1499,1500,1501]:
            add(f"session-idle:{word}:{delay}",typed(word)+drain()+[dict(op="timer",time=10000+(len(word)-1)*120+delay)]+typed("ka",time=12500)+drain())
    for interval in [500,600]:
        steps=[]
        for i, letter in enumerate("hima"):
            steps += typed(letter,time=10000+i*interval)+[dict(op="timer",time=10000+i*interval+100)]
        add(f"slow:{interval}",steps+drain())
    for phase, word in [("collecting","ka"),("flushing","konn"),("committed","konn")]:
        for vk in [8,9,13,27,32,0x10,0x11,0x25,0xbe,0xf3]:
            add(f"interrupt:{phase}:{vk}",typed(word)+(drain() if phase=="committed" else [])+press(vk,11000)+typed("u",11200)+drain())
    for vk in [0x15,0x16,0x19,0x1a,0x1c,0x1d,0xf0,0xf2,0xf3,0xf4]:
        for delay in [3999,4000,4001]:
            add(f"feedback:{vk}:{delay}",typed("konn")+drain()+press(vk,10360+delay)+press(32,15000),SessionIdleMs=60000)
    for phase,word in [("idle",""),("collecting","ka"),("flushing","konn"),("committed","konn")]:
        initial=typed(word)+(drain() if phase=="committed" else [])
        add("focus:"+phase,initial+[dict(op="context",time=11000)]+typed("ka",11200)+drain()+press(32,11800))
        add("abort:"+phase,initial+[dict(op="abort")]+drain()+typed("konn",11200)+drain())
        add("injected:"+phase,initial+press(0x41,11000,injected=True)+drain())
    for vk in [0x10,0x11,0x12,0xa0,0xa1,0xa2,0xa3,0xa4,0xa5,0x5b,0x5c]:
        add(f"modifier:{vk}",press(vk,10000)+typed("c",10100,modifierDown=True)+press(32,11000))
    add("permission:denied",typed("konn",allowed=False,reason="対象外アプリ")+drain()+press(32,11500))
    add("permission:changed-while-collecting",typed("k",allowed=True)+typed("onn",10200,allowed=False,reason="変更")+drain())
    add("flush:two-batches",typed("konn")+[dict(op="take")]+typed("ichi",10800)+[dict(op="take"),dict(op="take")]+typed("wa ",11400))
    add("flush:space-after-first-batch",typed("konn")+[dict(op="take")]+press(32,10800)+drain())
    for enabled in [False,True]:
        for boundary in [[],press(0x10,11100),press(0xbe,11100),[dict(op="context",time=11100)]]:
            add(f"continue:{enabled}:{len(cases)}",typed("hel")+drain()+typed("lo ",10600)+boundary+typed("kyou",11300)+drain(),ContinueEnglishAfterSpace=enabled)
    for seed in range(8):
        rng=random.Random(seed)
        time=10000
        steps=[]
        for _ in range(80):
            time += rng.choice([0,40,120,699,700,1501])
            op=rng.choice(["key"]*6+["timer","context","take","abort"])
            if op=="key":
                steps.append(dict(op=op,time=time,vk=rng.choice([0x4b,0x41,0x4e,0x4f,0x4c,0x15,0x20,0x10,0x25]),
                    scan=rng.randrange(128),extended=bool(rng.randrange(2)),isUp=rng.randrange(4)==0,
                    injected=rng.randrange(8)==0,modifierDown=rng.randrange(8)==0,allowed=rng.randrange(8)!=0,reason="test"))
            else:
                steps.append(dict(op=op,**({"time":time} if op in ["timer","context"] else {})))
        add(f"interleavings:{seed}",steps+drain())
    for word,outcome,decided in [("zanzo","StayedRejected",False),("kyo","JapaneseRejected",False),
            ("konnichiwa","JapaneseAccepted",False),("ka","JapaneseAccepted",False),
            ("github","StayedAccepted",True),("zanzo","StayedAccepted",False),
            ("UPPER","JapaneseAccepted",False),("abc1","StayedRejected",False)]:
        learn=[dict(letters=word,outcome=outcome,decidedEnglish=decided)]
        add(f"learn:{word}:{outcome}",typed(word if word.islower() else "konn")+drain()+press(32,15000))
        cases[-1]["learn"]=learn
    for enabled in [False,True]:
        add(f"learn:enabled:{enabled}",typed("zanzok")+drain(),LearningEnabled=enabled)
        cases[-1]["learn"]=[dict(letters="zanzo",outcome="StayedRejected",decidedEnglish=False)]
    for outcome in ["JapaneseAccepted","JapaneseRejected","StayedAccepted","StayedRejected"]:
        add("learn:cap:"+outcome,typed("konn")+drain())
        cases[-1]["learn"]=[dict(letters="konn",outcome=outcome,decidedEnglish=True,repeat=1100)]
    add("learn:longest-prefix",typed("zanzok")+drain())
    cases[-1]["learn"]=[dict(letters="zan",outcome="StayedRejected",decidedEnglish=False),
        dict(letters="zanzo",outcome="JapaneseRejected",decidedEnglish=False)]
    add("learn:existing-unknown",typed("zanzo")+drain())
    cases[-1]["learn"]=[dict(letters="zanzo",outcome="JapaneseAccepted",decidedEnglish=False),
        dict(letters="zanzo",outcome="StayedAccepted",decidedEnglish=False)]
    return cases


if __name__ == "__main__":
    path=Path(sys.argv[1])
    with path.open("x",encoding="utf-8") as out:
        json.dump(fixtures(),out,ensure_ascii=False,indent=2)
    print(f"{len(fixtures())} hook trace cases written")
