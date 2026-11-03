-------------------------------------------------------------------------------
-- bureau's project-local Neovim settings.
--
-- Neovim reads this file only when `exrc` is enabled and the file is trusted:
--
--     vim.opt.exrc = true
--
-- Add that line to your init.lua, then open Neovim in this repository and
-- approve the trust prompt. See `:help exrc`.
--
-- The settings apply to Markdown buffers under this repository only. They are
-- not inherited by a window showing some other file.
-------------------------------------------------------------------------------

local root = vim.fs.normalize(vim.fs.dirname(debug.getinfo(1, "S").source:sub(2)))
local group = vim.api.nvim_create_augroup("BureauNotes", { clear = true })

-- The width of a run of leading whitespace, with a tab advancing to the next
-- four-column stop, matching how `bureau tasks` reads nesting.
local function columns(whitespace)
  local width = 0
  for index = 1, #whitespace do
    if whitespace:sub(index, index) == "\t" then
      width = width + 4 - (width % 4)
    else
      width = width + 1
    end
  end
  return width
end

-- One fold level per line: headings set the base level, indentation adds one
-- level per four columns. Fenced code blocks are kept out of the calculation.
local function fold_levels(buffer)
  local lines = vim.api.nvim_buf_get_lines(buffer, 0, -1, false)
  local levels = {}
  local heading = 1
  local fence = false

  for index, line in ipairs(lines) do
    local trimmed = line:match("^%s*(.-)%s*$") or line
    if trimmed:match("^```") or trimmed:match("^~~~") then
      fence = not fence
    end

    local hashes = not fence and line:match("^(#+)%s")
    if hashes then
      heading = #hashes
      levels[index] = ">" .. heading
    elseif fence then
      levels[index] = tostring(heading)
    else
      local whitespace = line:match("^%s*") or ""
      levels[index] = tostring(math.min(heading + math.floor(columns(whitespace) / 4), 20))
    end
  end

  return levels
end

-- `foldexpr` is evaluated for every line, so cache the computed levels until
-- the buffer changes.
function _G.bureau_notes_fold(lnum)
  local buffer = vim.api.nvim_get_current_buf()
  local tick = vim.api.nvim_buf_get_changedtick(buffer)
  local cache = vim.b.bureau_notes_folds
  if not cache or cache.tick ~= tick then
    cache = { tick = tick, levels = fold_levels(buffer) }
    vim.b.bureau_notes_folds = cache
  end
  return cache.levels[lnum] or "0"
end

local function in_repository(buffer)
  local name = vim.api.nvim_buf_get_name(buffer)
  if name == "" then
    return false
  end
  name = vim.fs.normalize(name)
  local prefix = root .. "/"
  if vim.fn.has("win32") == 1 then
    name, prefix = name:lower(), prefix:lower()
  end
  return name:sub(1, #prefix) == prefix
end

local function apply()
  local buffer = vim.api.nvim_get_current_buf()
  if vim.bo[buffer].filetype ~= "markdown" or not in_repository(buffer) then
    return
  end

  -- Remember the window's own fold settings so leaving a bureau note can put
  -- them back.
  if not vim.w.bureau_notes_saved then
    vim.w.bureau_notes_saved = {
      foldmethod = vim.wo.foldmethod,
      foldexpr = vim.wo.foldexpr,
      foldlevel = vim.wo.foldlevel,
    }
  end

  -- Folds are computed from the note itself, not from a plugin.
  vim.wo.foldmethod = "expr"
  vim.wo.foldexpr = "v:lua.bureau_notes_fold(v:lnum)"
  -- Keep every fold open by default; use `zM` to close them all, `zo`/`zc` to
  -- open or close one. Set this to 2 to fold `###` subsections and nested
  -- lists as soon as the file opens.
  vim.wo.foldlevel = 99
end

local function restore()
  if in_repository(vim.api.nvim_get_current_buf()) then
    return
  end
  if vim.w.bureau_notes_saved then
    vim.wo.foldmethod = vim.w.bureau_notes_saved.foldmethod
    vim.wo.foldexpr = vim.w.bureau_notes_saved.foldexpr
    vim.wo.foldlevel = vim.w.bureau_notes_saved.foldlevel
    vim.w.bureau_notes_saved = nil
  end
end

vim.api.nvim_create_autocmd({ "BufEnter", "FileType" }, {
  group = group,
  callback = function()
    restore()
    apply()
  end,
})

apply()
