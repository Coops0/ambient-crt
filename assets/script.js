// i love you jquery
const $ = (selector) => document.querySelector(selector);

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
    <option value="none">~</option>
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
    return;
  }

  const videos = await response.json();
  videoList.innerHTML = videos
    .map(
      (video) => `
           <li class="video-item bg-black border-2 border-purple-500 rounded-md shadow overflow-hidden" data-video="${video.name}">
             <div class="relative">
               <img src="/thumbs/${video.name_without_ext}.jpg" alt="${video.name} thumbnail" class="w-full h-48 object-cover">
               <div class="absolute bottom-0 left-0 right-0 p-4 bg-gradient-to-t from-black to-transparent">
                 <span class="block text-purple-500 font-bold truncate">${video.name}</span>
               </div>
             </div>
             <div class="flex divide-x-2 divide-purple-500">
               <button class="video-button w-1/2 py-2 bg-green-500 hover:bg-green-600 text-black text-sm uppercase tracking-wider focus:outline-none focus:ring-2 focus:ring-green-500">Play</button>
               <button class="delete-button w-1/2 py-2 bg-red-500 hover:bg-red-600 text-black text-sm uppercase tracking-wider focus:outline-none focus:ring-2 focus:ring-red-500">Delete</button>
             </div>
             <div class="px-4 py-2 video-select-parent hidden">
                <input type="checkbox" class="video-select form-checkbox h-6 w-6 text-purple-500 border-2 border-purple-500 rounded-md focus:ring-purple-500">
             </div>
           </li>
         `,
    )
    .join("");
}

async function playVideo(videoName) {
  await fetch("/videos", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      video_name: videoName,
      ...getSettings(),
    }),
  });
}

async function stopVideo() {
  await fetch("/stop");
}

async function uploadVideo(file, fileName) {
  const response = await fetch(`/videos?video_name=${fileName}`, {
    method: "POST",
    body: file,
  });

  if (response.ok) {
    fetchVideos();
  }
}

async function deleteVideo(videoName) {
  const response = await fetch(`/videos`, {
    method: "DELETE",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ video_name: videoName }),
  });

  if (response.ok) {
    fetchVideos();
  }
}

// pass in playlistName as undefined to shuffle all
async function playPlaylist(playlistName) {
  await fetch("/playlists", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      playlist_name: playlistName,
      ...getSettings(),
    }),
  });
}

// videos = [] to delete
async function savePlaylist(playlistName, videos) {
  await fetch("/playlists", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      playlist_name: playlistName,
      videos,
    }),
  });

  await fetchPlaylists();
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

$("#randomButton").addEventListener("click", async () => {
  const response = await fetch("/videos");
  if (!response.ok) {
    return;
  }

  const videos = await response.json();
  const randomVideo = videos[Math.floor(Math.random() * videos.length)];

  playVideo(randomVideo.name);
});

$("#shuffleButton").addEventListener("click", async () => {
  // backend handles this
  await playPlaylist(undefined);
});

$("#stopButton").addEventListener("click", stopVideo);

$("#playlist").addEventListener("change", async (e) => {
  const playlistName = e.target.value;

  $("#newPlaylistName").value = "";

  if (!playlistName || playlistName === "none") {
    // if deselect, deselect all and hide new playlist section
    selectVideosFromPlaylist(""); // clear selection
    playlistUtilVisible(false);
    return;
  }

  playlistUtilVisible(true);
  if (playlistName !== "new") {
    // existing, select and set name in case of changes
    $("#newPlaylistName").value = playlistName.replaceAll("_", " ");
    selectVideosFromPlaylist(playlistName);
  }
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

function selectVideosFromPlaylist(playlistName) {
  const playlist = playlists.get(playlistName) || [];

  document.querySelectorAll(".video-item").forEach((item) => {
    const checkbox = item.querySelector(".video-select");
    checkbox.checked = playlist.includes(item.dataset.video);
  });
}

fetchVideos();
fetchPlaylists();
