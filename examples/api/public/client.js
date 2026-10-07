// The API client: plain JavaScript, no framework. Everything goes through
// `api()`, which adds the token and turns error replies into exceptions.
"use strict";

const TOKEN = "api-token";
let cursor = null;

async function api(method, path, body) {
  const headers = { accept: "application/json" };
  const token = sessionStorage.getItem(TOKEN);
  if (token) headers.authorization = "Bearer " + token;
  if (body) headers["content-type"] = "application/json";
  const res = await fetch(path, { method, headers, body: body && JSON.stringify(body) });
  if (res.status === 401 && path !== "/api/tokens") {
    // Expired or revoked: back to the login form.
    sessionStorage.removeItem(TOKEN);
    show();
    throw { message: "Your session ended; log in again." };
  }
  const data = res.status === 204 ? null : await res.json().catch(() => null);
  if (!res.ok) throw data || { message: res.status + " " + res.statusText };
  return data;
}

// A 422 reply: {"message", "errors": {"field": ["…"]}} next to the fields.
function showErrors(form, err) {
  form.querySelectorAll("[data-error-for]").forEach((p) => {
    const message = (err.errors && err.errors[p.dataset.errorFor] || [])[0] || "";
    p.textContent = message;
    // The kit marks the field too, for its red border and screen readers.
    const field = form.elements[p.dataset.errorFor];
    if (field) field.toggleAttribute("aria-invalid", !!message);
  });
  const general = form.querySelector("[data-error]");
  general.textContent = err.errors ? "" : err.message || "";
}

async function show() {
  const loggedIn = !!sessionStorage.getItem(TOKEN);
  document.getElementById("login").hidden = loggedIn;
  document.getElementById("app").hidden = !loggedIn;
  if (!loggedIn) return;
  const me = await api("GET", "/api/me");
  document.getElementById("who").textContent =
    me.user.name + " · " + me.abilities.join(", ");
  document.getElementById("add").hidden = !me.abilities.includes("products:write");
  document.getElementById("products").replaceChildren();
  cursor = null;
  await loadMore(me.abilities.includes("products:write"));
}

// Prices travel in cents (an integer): 450 is $4.50.
function money(cents) {
  return (Number(cents) / 100).toLocaleString("en-US", { style: "currency", currency: "USD" });
}

// A row of the kit's `list`: the name takes the room, then the price and
// a Delete button (the kit's classes, so it looks like the rest of the page).
function row(product, canWrite) {
  const li = document.createElement("li");
  const name = document.createElement("span");
  name.className = "rx-list__main";
  name.textContent = product.name;
  const price = document.createElement("span");
  price.className = "rx-subtitle";
  price.textContent = money(product.price);
  li.append(name, price);
  if (canWrite) {
    const del = document.createElement("button");
    del.className = "rx-button rx-button--plain-danger rx-button--small";
    del.type = "button";
    del.textContent = "Delete";
    del.addEventListener("click", async () => {
      await api("DELETE", "/api/products/" + product.id);
      li.remove();
    });
    li.append(del);
  }
  return li;
}

// The list is cursor-paginated: `next_cursor` asks for the next page.
async function loadMore(canWrite) {
  const page = await api("GET", "/api/products" + (cursor ? "?cursor=" + encodeURIComponent(cursor) : ""));
  const list = document.getElementById("products");
  page.items.forEach((p) => list.append(row(p, canWrite)));
  cursor = page.next_cursor;
  const more = document.getElementById("more");
  more.hidden = !cursor;
  more.onclick = () => loadMore(canWrite);
}

document.addEventListener("DOMContentLoaded", () => {
  document.getElementById("login").addEventListener("submit", async (event) => {
    event.preventDefault();
    const form = event.target;
    try {
      const reply = await api("POST", "/api/tokens", {
        email: form.elements.email.value,
        password: form.elements.password.value,
        device: "Browser client",
        read_only: form.elements.read_only.checked,
      });
      sessionStorage.setItem(TOKEN, reply.token);
      showErrors(form, {});
      await show();
    } catch (err) {
      showErrors(form, err.errors ? err : { message: err.message || "Wrong email or password." });
    }
  });

  document.getElementById("add").addEventListener("submit", async (event) => {
    event.preventDefault();
    const form = event.target;
    try {
      const product = await api("POST", "/api/products", {
        name: form.elements.name.value,
        price: Math.round(Number(form.elements.price.value) * 100),
      });
      showErrors(form, {});
      form.reset();
      document.getElementById("products").prepend(row(product, true));
    } catch (err) {
      showErrors(form, err.errors ? err : { message: err.message || "Something went wrong." });
    }
  });

  document.getElementById("logout").addEventListener("click", async () => {
    // The token stops working on the server, then the page forgets it.
    await api("DELETE", "/api/tokens/current").catch(() => {});
    sessionStorage.removeItem(TOKEN);
    show();
  });

  show().catch(() => {});
});
