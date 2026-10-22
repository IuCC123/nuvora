"""Archivo.ttf, the font the episode fixture "embeds": a static Bold, semi-condensed
instance of Archivo (SIL Open Font License) cut from the fontsource variable font
the app already ships, renamed to plain "Archivo" as a release's attachment would be.
Run: python3 -m pip install fonttools brotli && python3 make-font.py"""
import os
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

here = os.path.dirname(os.path.abspath(__file__))
source = os.path.join(here, "../../node_modules/@fontsource-variable/archivo/files/archivo-latin-wdth-normal.woff2")
font = instantiateVariableFont(TTFont(source), {"wght": 700, "wdth": 87})
font.flavor = None
names = font["name"]
for record in list(names.names):
    if record.nameID in (16, 17):
        names.removeNames(nameID=record.nameID)
for name_id, value in {1: "Archivo", 2: "Bold", 4: "Archivo Bold", 6: "Archivo-Bold"}.items():
    names.setName(value, name_id, 3, 1, 0x409)
    names.setName(value, name_id, 1, 0, 0)
font["OS/2"].usWeightClass = 700
font["OS/2"].fsSelection = (font["OS/2"].fsSelection | 0x20) & ~0x40
font["head"].macStyle |= 1
font.save(os.path.join(here, "Archivo.ttf"))
