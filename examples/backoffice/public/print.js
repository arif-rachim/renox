// The Print button on the printed invoice (a file, as the CSP allows no
// inline scripts).
document.addEventListener("click", function (event) {
  if (event.target.closest("[data-print]")) window.print();
});
