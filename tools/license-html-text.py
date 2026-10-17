#!/usr/bin/env python3
"""Convert provided HTML copyright notices to selectable text at build time."""
from html.parser import HTMLParser
import re
import sys

class NoticeText(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.parts=[]
        self.hidden=0
        self.blocks={'p','div','section','article','li','ul','ol','pre','h1','h2','h3','h4','br','tr'}
        self.anchor=None
    def handle_starttag(self,tag,attrs):
        if tag in {'head','style','script'}:
            self.hidden+=1
        if self.hidden:
            return
        if tag in self.blocks:
            self.parts.append('\n')
        if tag=='a':
            href=dict(attrs).get('href','')
            self.anchor=(href,len(self.parts)) if href.startswith(('http://','https://')) else None
    def handle_endtag(self,tag):
        if tag in {'head','style','script'}:
            self.hidden=max(0,self.hidden-1)
            return
        if self.hidden:
            return
        if tag=='a' and self.anchor:
            href,index=self.anchor
            if href not in ''.join(self.parts[index:]):
                self.parts.append(' ('+href+')')
            self.anchor=None
        if tag in self.blocks:
            self.parts.append('\n')
    def handle_data(self,data):
        if not self.hidden:
            self.parts.append(data)

parser=NoticeText()
parser.feed(sys.stdin.read())
parser.close()
text='\n'.join(line.rstrip() for line in ''.join(parser.parts).splitlines())
text=re.sub(r'\n(?:[ \t]*\n){2,}','\n\n',text).strip()
sys.stdout.write(text+'\n')
