// i love you jquery
const $ = (selector) => document.querySelector(selector);
const notyf = new Notyf();

const fileInput = $("#fileInput");
const fileNameInput = $("#fileNameInput");
const videoList = $("#videoList");

const playlists = new Map();

function getSettings() {
  const visualizer = $("#visualizer").value;

  return {
    gain: parseFloat($("#gain").value),
    visualizer: visualizer === "none" ? undefined : visualizer,
  };
}

async function fetchPlaylists() {
  const response = await fetch("/playlists");
  if (!response.ok) {
    return;
  }

  const playlistResponse = await response.json();

  playlists.clear();
  playlistResponse.forEach(({ name, videos }) => playlists.set(name, videos));

  $("#playlist").innerHTML = `
    <option value="none">select playlist</option>
    <option value="new">new</option>
    ${playlistResponse
      .map(
        (p) =>
          `<option value="${p.name}">${p.name.replaceAll("_", " ")}</option>`,
      )
      .join("")}
  `;
}

async function fetchVideos() {
  const response = await fetch("/videos");
  if (!response.ok) {
    console.error("Failed to fetch videos", response);
    return;
  }

  const videos = await response.json();
  videoList.innerHTML = videos
    .map(
      (video) => `
             <li class="video-item bg-black rounded-md shadow overflow-hidden relative" data-video="${video.name}">
               <div class="absolute top-2 left-2 z-10 video-select-parent hidden">
                 <label class="inline-flex items-center">
                   <input type="checkbox" class="video-select form-checkbox h-5 w-5 appearance-none border-2 border-purple-500 rounded-none bg-black checked:bg-purple-500 focus:outline-none focus:ring-2 focus:ring-purple-500 transition duration-200">
                 </label>
               </div>
               <div class="relative">
                 <img src="/thumbs/${video.name_without_ext}.jpg" alt="${video.name}" class="w-full h-48 object-cover">
                 <div class="video-name absolute bottom-0 left-0 right-0 p-2 bg-black bg-opacity-70 text-purple-500 text-sm truncate">${video.name}</div>
               </div>
               <div class="flex divide-x-2 transition-all">
                 <button class="video-button w-1/2 py-2 bg-green-500 hover:bg-green-600 text-black text-sm uppercase tracking-wider focus:outline-none focus:ring-2 focus:ring-green-500">Play</button>
                 <button class="delete-button w-1/2 py-2 bg-red-500 hover:bg-red-600 text-black text-sm uppercase tracking-wider focus:outline-none focus:ring-2 focus:ring-red-500">Delete</button>
               </div>
             </li>
           `,
    )
    .join("");
}

async function playVideo(videoName) {
  const response = await fetch("/videos", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      video_name: videoName,
      ...getSettings(),
    }),
  });

  if (!response.ok) {
    notyf.error("Failed to play video");
    console.error(response);
  }
}

async function stopVideo() {
  const response = await fetch("/stop");
  if (!response.ok) {
    notyf.error("Failed to stop video");
    console.error(response);
  }
}

async function uploadVideo(file, fileName) {
  const response = await fetch(`/videos?video_name=${fileName}`, {
    method: "POST",
    body: file,
  });

  if (response.ok) {
    const path = await response.text();
    notyf.success(`Video uploaded to ${path}`);

    await fetchVideos();
  } else {
    notyf.error("Failed to upload video");
    console.error(response);
  }
}

async function deleteVideo(videoName) {
  const response = await fetch(`/videos`, {
    method: "DELETE",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ video_name: videoName }),
  });

  if (response.ok) {
    notyf.success("Video deleted");
    fetchVideos();
  } else {
    notyf.error("Failed to delete video");
    console.error(response);
  }
}

async function playMedia(url) {
  const response = await fetch("/custom-media", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      url,
      ...getSettings(),
    }),
  });

  if (response.ok) {
    notyf.success("Playing custom media");
  } else {
    notyf.error("Failed to play media");
    console.error(response);
  }
}

// pass in playlistName as undefined to shuffle all
async function playPlaylist(playlistName) {
  const response = await fetch("/playlists", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      playlist_name: playlistName,
      ...getSettings(),
    }),
  });

  if (!response.ok) {
    notyf.error("Failed to play playlist");
    console.error(response);
  }
}

// videos = [] to delete
async function savePlaylist(playlistName, videos) {
  const response = await fetch("/playlists", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      playlist_name: playlistName,
      videos,
    }),
  });

  if (response.ok) {
    notyf.success("Playlist saved");
    await fetchPlaylists();
  } else {
    notyf.error("Failed to save playlist");
    console.error(response);
  }
}

// 0 = play/pause
// 1 = skip
// 2 = back
async function pressMediaKey(action) {
  const response = await fetch("/media-control", {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ action }),
  });

  if (!response.ok) {
    notyf.error("Failed to send media key");
    console.error(response);
  }
}

$("#browseButton").addEventListener("click", () => fileInput.click());

$("#uploadForm").addEventListener("submit", async (e) => {
  e.preventDefault();

  const file = fileInput.files[0];
  const fileName = fileNameInput.value;

  if (file) {
    uploadVideo(file, fileName);
  }
});

fileInput.addEventListener("change", (e) => {
  const file = e.target.files[0];
  if (file) {
    fileNameInput.value = file.name;
  }
});

videoList.addEventListener("click", (e) => {
  const { classList } = e.target;
  const { video: videoName } = e.target?.parentNode?.parentNode?.dataset;

  if (!videoName) {
    return;
  }

  if (classList.contains("video-button")) {
    playVideo(videoName);
  }

  if (classList.contains("delete-button")) {
    deleteVideo(videoName);
  }
});

$("#shuffleButton").addEventListener("click", async () => {
  // backend handles this
  await playPlaylist(undefined);
});

$("#stopButton").addEventListener("click", stopVideo);

$("#playlist").addEventListener("change", async (e) => {
  const playlistName = e.target.value;

  $("#newPlaylistName").value = "";
  selectVideosFromPlaylist("");

  if (!playlistName || playlistName === "none") {
    // if deselect, deselect all and hide new playlist section
    return playlistUtilVisible(false);
  }

  playlistUtilVisible(true);
  if (playlistName === "new") {
    return;
  }

  // existing playlist, select and set name in case of changes
  $("#newPlaylistName").value = playlistName.replaceAll("_", " ");
  selectVideosFromPlaylist(playlistName);
});

function playlistUtilVisible(visible) {
  for (const element of [
    $("#newPlaylistSection"),
    $("#playlistActions"),
    ...document.querySelectorAll(".video-select-parent"),
  ]) {
    element.classList.toggle("hidden", !visible);
  }
}

$("#playPlaylistButton").addEventListener("click", async () => {
  const playlistName = $("#playlist").value;
  if (playlistName && playlistName !== "new" && playlistName !== "none") {
    await playPlaylist(playlistName);
  }
});

$("#savePlaylistButton").addEventListener("click", async () => {
  const playlistName = $("#newPlaylistName").value;
  if (!playlistName) {
    return;
  }

  const videos = Array.from(
    document.querySelectorAll(".video-select:checked"),
  ).map((checkbox) => checkbox.closest(".video-item").dataset.video);

  await savePlaylist(playlistName, videos);
  await fetchPlaylists();

  $("#playlist").value = playlistName.replaceAll(" ", "_");
});

$("#deselectButton").addEventListener("click", () =>
  selectVideosFromPlaylist(""),
);

$("#selectAllButton").addEventListener("click", () => {
  document
    .querySelectorAll(".video-select")
    .forEach((checkbox) => (checkbox.checked = true));
});

let listeningForPaste = false;

$("#playMediaButton").addEventListener("click", () => {
  listeningForPaste = true;
  $("#pasteBackdrop").classList.remove("hidden");
});

document.addEventListener("paste", (e) => {
  if (!listeningForPaste) {
    return;
  }

  const url = e.clipboardData?.getData("text") || "";
  if (url.trim().length === 0) {
    return;
  }

  $("#pasteBackdrop").classList.add("hidden");
  playMedia(url);
});

document.addEventListener("keydown", async (e) => {
  if (listeningForPaste && e.key === "Escape") {
    listeningForPaste = false;
    $("#pasteBackdrop").classList.add("hidden");
  }
});

function selectVideosFromPlaylist(playlistName) {
  const playlist = playlists.get(playlistName) || [];

  document.querySelectorAll(".video-item").forEach((item) => {
    const checkbox = item.querySelector(".video-select");
    checkbox.checked = playlist.includes(item.dataset.video);
  });
}

$("#playPauseButton").addEventListener("click", () => pressMediaKey(0));
$("#nextButton").addEventListener("click", () => pressMediaKey(1));
$("#backButton").addEventListener("click", () => pressMediaKey(2));

fetchVideos();
fetchPlaylists();
