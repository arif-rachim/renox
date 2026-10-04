#!/usr/bin/env python3
"""Uses an app made by `rnx new` over HTTP, the way a person in a browser does.

The generated apps' own tests drive them through `TestApp`; this script talks to the
running server instead: it reads the forms off the pages (their action, method,
`_method` and CSRF fields), keeps the session cookie, follows redirects, and checks
what the next page says. A generator that writes code which compiles and passes its
own tests but doesn't work in a browser fails here.

    tests/cli/smoke.py resource <base url> <path>   # a `make:module --resource` module
    tests/cli/smoke.py starter <base url>           # an app made by `rnx new --starter`

Run by tests/cli/run.sh after it starts the app. Only the standard library is used.
"""

import html
import http.cookiejar
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from html.parser import HTMLParser

STEP = "start"


def fail(message, page=None):
    print(f"\nsmoke: FAIL at step {STEP!r}: {message}", file=sys.stderr)
    if page is not None:
        print(f"  {page.status} {page.url}", file=sys.stderr)
        print(page.text[:2000], file=sys.stderr)
    sys.exit(1)


def step(name):
    global STEP
    STEP = name
    print(f"--   {name}")


class Forms(HTMLParser):
    """Every <form> on a page: its attributes and the fields it would send."""

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.forms = []
        self.form = None
        self.textarea = None
        self.select = None

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if tag == "form":
            self.form = {"action": a.get("action", ""), "method": a.get("method", "get"),
                         "fields": {}, "names": []}
            self.forms.append(self.form)
            return
        if self.form is None:
            return
        name = a.get("name")
        if tag == "input" and name:
            kind = a.get("type", "text")
            self.form["names"].append(name)
            if kind in ("checkbox", "radio"):
                if "checked" in a:
                    self.form["fields"][name] = a.get("value", "on")
            elif kind not in ("submit", "button", "file"):
                self.form["fields"][name] = a.get("value", "")
        elif tag == "textarea" and name:
            self.form["names"].append(name)
            self.form["fields"][name] = ""
            self.textarea = name
        elif tag == "select" and name:
            self.form["names"].append(name)
            self.select = name
        elif tag == "option" and self.select:
            if "selected" in a or self.select not in self.form["fields"]:
                self.form["fields"][self.select] = a.get("value", "")

    def handle_data(self, data):
        if self.textarea:
            self.form["fields"][self.textarea] += data

    def handle_endtag(self, tag):
        if tag == "form":
            self.form = None
        elif tag == "textarea":
            self.textarea = None
        elif tag == "select":
            self.select = None


class Page:
    def __init__(self, status, url, text):
        self.status, self.url, self.text = status, url, text

    @property
    def path(self):
        return urllib.parse.urlparse(self.url).path

    def forms(self):
        parser = Forms()
        parser.feed(self.text)
        return parser.forms

    def form(self, action=None, method=None, field=None):
        """The first form with that action, spoofed method (`_method`) or field."""
        for form in self.forms():
            if action is not None and urllib.parse.urlparse(form["action"]).path != action:
                continue
            if method is not None and form["fields"].get("_method", "").upper() != method:
                continue
            if field is not None and field not in form["names"]:
                continue
            return form
        fail(f"no form (action={action}, method={method}, field={field}) on the page", self)

    def see(self, text):
        if text not in html.unescape(self.text):
            fail(f"expected to see {text!r}", self)
        return self

    def dont_see(self, text):
        if text in html.unescape(self.text):
            fail(f"expected not to see {text!r}", self)
        return self


class Browser:
    def __init__(self, base):
        self.base = base.rstrip("/")
        self.cookies = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(self.cookies))

    def request(self, path, data=None):
        url = path if path.startswith("http") else self.base + path
        body = urllib.parse.urlencode(data).encode() if data is not None else None
        req = urllib.request.Request(url, data=body, headers={"Accept": "text/html"})
        try:
            with self.opener.open(req, timeout=30) as res:
                return Page(res.status, res.geturl(), res.read().decode("utf-8", "replace"))
        except urllib.error.HTTPError as err:
            return Page(err.code, err.geturl(), err.read().decode("utf-8", "replace"))

    def get(self, path, status=200):
        page = self.request(path)
        if status is not None and page.status != status:
            fail(f"GET {path} answered {page.status}, not {status}", page)
        return page

    def submit(self, form, values=None, status=200):
        """Sends a form as the browser would (POST with its hidden fields), after
        following the redirect it answers with."""
        fields = dict(form["fields"])
        for name, value in (values or {}).items():
            if value is None:
                fields.pop(name, None)
            else:
                fields[name] = value
        action = form["action"] or "/"
        if form["method"].lower() == "get":
            page = self.request(action + "?" + urllib.parse.urlencode(fields))
        else:
            page = self.request(action, fields)
        if status is not None and page.status != status:
            fail(f"{form['method'].upper()} {action} ended at {page.status}, not {status}", page)
        return page


def register(browser, name, email):
    page = browser.get("/register")
    return browser.submit(page.form(action="/register"), {
        "name": name, "email": email,
        "password": "correct-horse-battery", "password_confirmation": "correct-horse-battery",
    })


def login(browser, email, password):
    page = browser.get("/login")
    return browser.submit(page.form(action="/login"), {"email": email, "password": password})


def example_value(name):
    """A value a person might type, by the field's name."""
    if name.endswith("_on") or name.endswith("date"):
        return "2026-10-04"
    if name.endswith("_at"):
        return "2026-10-04T09:30"
    if name in ("price", "amount", "total", "quantity", "stock") or name.endswith("_id"):
        return "12500" if not name.endswith("_id") else "1"
    if name in ("active",) or name.startswith("is_"):
        return "on"
    if name == "email":
        return "smoke@example.com"
    return f"Smoke {name}"


def resource(base, path):
    """Register, then list, create, show, edit and delete one record."""
    browser = Browser(base)

    step(f"a guest is sent to /login from {path}")
    page = browser.get(path)
    if page.path != "/login":
        fail(f"a guest reached {page.path}", page)

    step("register (a form with its CSRF token)")
    page = register(browser, "Smoke Tester", f"smoke{path.replace('/', '-')}@example.com")
    if page.path in ("/register", "/login"):
        fail(f"registering ended on {page.path}", page)

    step(f"the empty list at {path}")
    page = browser.get(path)
    if page.path != path:
        fail(f"a registered person was sent to {page.path}", page)

    step(f"the form at {path}/new")
    page = browser.get(f"{path}/new")
    form = page.form(action=path)
    editable = [n for n in form["names"] if not n.startswith("_")]
    if not editable:
        fail("the new form has no fields", page)
    values = {n: example_value(n) for n in editable}
    first = editable[0]

    step("create (POST, redirected back)")
    page = browser.submit(form, values)
    page = browser.get(path).see(values[first])

    step("show the record")
    created = None
    for link in sorted(set(_links(page.text, path)), key=len):
        rest = link[len(path) + 1:]
        if rest and "/" not in rest and rest != "new":
            created = link
            break
    if not created:
        fail(f"no link to the record under {path}/…", page)
    browser.get(created).see(values[first])

    step("edit (PUT through _method)")
    page = browser.get(f"{created}/edit")
    form = page.form(action=created, method="PUT")
    renamed = values[first] + " (edited)"
    browser.submit(form, {first: renamed})
    browser.get(created).see(renamed)

    step("an invalid edit is refused and keeps the old value")
    page = browser.get(f"{created}/edit")
    browser.submit(page.form(action=created, method="PUT"), {first: ""}, status=None)
    browser.get(created).see(renamed)

    step("delete (DELETE through _method)")
    page = browser.get(created)
    form = page.form(action=created, method="DELETE")
    browser.submit(form)
    browser.get(path).dont_see(renamed)
    browser.get(created, status=404)

    step("log out")
    page = browser.get(path)
    browser.submit(page.form(action="/logout"))
    if browser.get(path).path != "/login":
        fail("still logged in after logging out")


def _links(text, prefix):
    for href in re.findall(r'href="([^"]+)"', text):
        href = html.unescape(href)
        if href.startswith(prefix + "/"):
            yield href.split("?")[0].split("#")[0]


def starter(base):
    """Sign-up lands on email verification; the seeded admin sees /users and /activity,
    the seeded member gets 403."""
    step("sign up lands on the email verification page")
    browser = Browser(base)
    page = register(browser, "New Person", "new-person@example.com")
    if "verif" not in page.path:
        fail(f"sign-up landed on {page.path}, not the verification notice", page)
    page = browser.get("/dashboard", status=None)
    if "verif" not in page.path:
        fail(f"an unverified person reached {page.path}", page)

    step("the seeded admin sees the dashboard, /users and /activity")
    admin = Browser(base)
    page = login(admin, "admin@example.com", "password123")
    if page.path != "/dashboard":
        fail(f"the admin landed on {page.path}", page)
    admin.get("/users").see("member@example.com")
    admin.get("/activity")

    step("the seeded member gets 403 at /users and /activity")
    member = Browser(base)
    page = login(member, "member@example.com", "password123")
    if page.path != "/dashboard":
        fail(f"the member landed on {page.path}", page)
    member.get("/users", status=403)
    member.get("/activity", status=403)

    step("a wrong password is refused")
    stranger = Browser(base)
    page = login(stranger, "admin@example.com", "wrong-password")
    if stranger.get("/dashboard", status=None).path != "/login":
        fail("a wrong password logged in", page)


def main(args):
    if len(args) >= 3 and args[0] == "resource":
        resource(args[1], args[2])
    elif len(args) >= 2 and args[0] == "starter":
        starter(args[1])
    else:
        sys.exit(__doc__)
    print(f"smoke: {args[0]} ok")


if __name__ == "__main__":
    main(sys.argv[1:])
