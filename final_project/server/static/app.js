"use strict";
const $ = (id) => document.getElementById(id);
let ws = null;
let my = JSON.parse(localStorage.getItem("c4") || "null"); // {code, token, seat, local}
let spectator = false;
let lastState = null;
let lastLine = [];
let localPending = false;

const nameVal = () => $("name").value.trim() || "anon";
const send = (o) => ws && ws.readyState === 1 && ws.send(JSON.stringify(o));

function connect() {
  ws = new WebSocket(`ws://${location.host}/ws`);
  ws.onopen = () => {
    if (my && my.token) {
      send({ type: "JoinRoom", code: my.code, name: nameVal(), token: my.token });
    } else {
      send({ type: "ListRooms" });
    }
  };
  ws.onmessage = (e) => handle(JSON.parse(e.data));
  ws.onclose = () => setTimeout(connect, 1000);
}

function handle(m) {
  switch (m.type) {
    case "RoomList":
      renderRooms(m.rooms);
      break;
    case "Joined":
      spectator = m.seat === 255;
      if (!spectator) {
        const wasLocal = !!(my && my.local && my.code === m.code);
        my = { code: m.code, token: m.token, seat: m.seat, local: localPending || wasLocal };
        localPending = false;
        localStorage.setItem("c4", JSON.stringify(my));
      }
      $("room-code").textContent = `room ${m.code}`;
      $("lobby").classList.add("hidden");
      $("game").classList.remove("hidden");
      lastLine = [];
      render(m.state);
      break;
    case "State":
      if (m.state.status === "playing" && lastState && lastState.status === "over") {
        lastLine = [];
        $("banner").classList.add("hidden");
        $("eval-line").textContent = "";
      }
      render(m.state);
      break;
    case "MovePlayed":
      if (m.eval && $("show-eval").checked) $("eval-line").textContent = m.eval;
      break;
    case "GameOver":
      lastLine = m.line;
      render(lastState);
      $("banner-text").textContent =
        m.winner === 0 ? "Draw!" : `${lastState.names[m.winner - 1]} wins!`;
      $("rematch").classList.toggle("hidden", spectator);
      $("banner").classList.remove("hidden");
      break;
    case "Error":
      toast(m.msg);
      if (m.msg === "no such room") leave();
      break;
  }
}

function myColor(state) {
  if (spectator || !my) return 0;
  if (my.local) return state.turn; // always "your" color in local mode
  return my.seat === state.p1_seat ? 1 : 2;
}

function render(state) {
  if (!state) return;
  lastState = state;
  const board = $("board");
  board.innerHTML = "";
  const mine = myColor(state);
  const myTurn = state.status === "playing" && state.turn === mine;
  for (let row = 5; row >= 0; row--) {
    for (let col = 0; col < 7; col++) {
      const cell = document.createElement("div");
      cell.className = "cell";
      const v = state.board[row][col];
      if (v) cell.classList.add(v === 1 ? "p1" : "p2");
      if (lastLine.some(([c, r]) => c === col && r === row)) cell.classList.add("win");
      if (myTurn && state.board[5][col] === 0) {
        cell.classList.add("playable");
        cell.dataset.col = col;
        cell.onclick = () => send({ type: "Move", col });
      }
      board.appendChild(cell);
    }
  }
  const names = state.names;
  $("turn-label").textContent =
    state.status === "waiting" ? "waiting for opponent…"
    : state.status === "over" ? "game over"
    : myTurn ? "your turn"
    : `${names[state.turn - 1]}'s turn`;
}

function renderRooms(rooms) {
  const ul = $("rooms");
  ul.innerHTML = "";
  for (const r of rooms) {
    const li = document.createElement("li");
    li.innerHTML = `<span>${r.code} — ${r.host}${r.vs_bot ? " \u{1F916}" : ""}</span>
                    <span>${r.open ? "join" : "spectate"}</span>`;
    li.onclick = () =>
      send(r.open ? { type: "JoinRoom", code: r.code, name: nameVal() }
                  : { type: "Spectate", code: r.code });
    ul.appendChild(li);
  }
}

function leave() {
  localStorage.removeItem("c4");
  my = null;
  spectator = false;
  lastState = null;
  lastLine = [];
  $("game").classList.add("hidden");
  $("lobby").classList.remove("hidden");
  $("banner").classList.add("hidden");
  $("eval-line").textContent = "";
  if (ws) ws.close(); // reconnect sends ListRooms since my is null
}

let toastTimer = null;
function toast(msg) {
  $("toast").textContent = msg;
  $("toast").classList.remove("hidden");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => $("toast").classList.add("hidden"), 3000);
}

$("create-human").onclick = () =>
  send({ type: "CreateRoom", name: nameVal(), vs_bot: false });
$("create-bot").onclick = () =>
  send({ type: "CreateRoom", name: nameVal(), vs_bot: true, bot_first: $("bot-first").checked });
$("create-local").onclick = () => {
  localPending = true;
  send({ type: "CreateRoom", name: nameVal(), vs_bot: false, local: true });
};
$("join-btn").onclick = () =>
  send({ type: "JoinRoom", code: $("join-code").value.toUpperCase(), name: nameVal() });
$("spectate-btn").onclick = () =>
  send({ type: "Spectate", code: $("join-code").value.toUpperCase() });
$("rematch").onclick = () => send({ type: "Rematch" });
$("leave").onclick = leave;
$("home-btn").onclick = leave;

connect();
