from __future__ import annotations

from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.parse import urlparse
from urllib.request import Request, urlopen
import xml.etree.ElementTree as ET

from .models import RawItem


USER_AGENT = "TianJi/0.1 (+local-first one-shot MVP)"
ATOM_NS = {"atom": "http://www.w3.org/2005/Atom"}


class TianJiInputError(ValueError):
    pass


def read_fixture(path: str | Path) -> str:
    fixture_path = Path(path)
    try:
        return fixture_path.read_text(encoding="utf-8")
    except (FileNotFoundError, PermissionError, UnicodeDecodeError, OSError) as error:
        raise TianJiInputError(
            f"Failed to read fixture file: {fixture_path}"
        ) from error


def fetch_url(url: str, timeout: float = 15.0) -> str:
    request = Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urlopen(request, timeout=timeout) as response:
            return response.read().decode("utf-8", errors="replace")
    except (HTTPError, URLError, TimeoutError, UnicodeDecodeError, OSError) as error:
        raise TianJiInputError(f"Failed to fetch source URL: {url}") from error


def source_name_from_url(url: str) -> str:
    parsed = urlparse(url)
    return parsed.netloc or parsed.path or "fixture"


def parse_feed(feed_text: str, source: str) -> list[RawItem]:
    try:
        root = ET.fromstring(feed_text)
    except ET.ParseError as error:
        raise TianJiInputError(f"Failed to parse feed for source: {source}") from error

    channel = root.find("channel")
    if channel is not None:
        return _parse_rss(channel, source)

    if root.tag.endswith("feed"):
        return _parse_atom(root, source)

    raise TianJiInputError(
        f"Unsupported feed format for source {source}: expected RSS or Atom"
    )


def _text(element: ET.Element | None, tag: str) -> str:
    if element is None:
        return ""
    found = element.find(tag)
    if found is None or found.text is None:
        return ""
    return found.text.strip()


def _parse_rss(channel: ET.Element, source: str) -> list[RawItem]:
    items: list[RawItem] = []
    for item in channel.findall("item"):
        title = _text(item, "title")
        summary = _text(item, "description")
        link = _text(item, "link")
        published_at = _text(item, "pubDate") or None
        if not title:
            continue
        items.append(
            RawItem(
                source=source,
                title=title,
                summary=summary,
                link=link,
                published_at=published_at,
            )
        )
    return items


def _parse_atom(root: ET.Element, source: str) -> list[RawItem]:
    items: list[RawItem] = []
    for entry in root.findall("atom:entry", ATOM_NS):
        title = _text_ns(entry, "atom:title")
        summary = _text_ns(entry, "atom:summary") or _text_ns(entry, "atom:content")
        published_at = (
            _text_ns(entry, "atom:published") or _text_ns(entry, "atom:updated") or None
        )
        link = ""
        link_el = entry.find("atom:link", ATOM_NS)
        if link_el is not None:
            link = link_el.attrib.get("href", "")
        if not title:
            continue
        items.append(
            RawItem(
                source=source,
                title=title,
                summary=summary,
                link=link,
                published_at=published_at,
            )
        )
    return items


def _text_ns(element: ET.Element | None, tag: str) -> str:
    if element is None:
        return ""
    found = element.find(tag, ATOM_NS)
    if found is None:
        return ""
    return "".join(found.itertext()).strip()
