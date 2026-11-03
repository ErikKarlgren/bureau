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

-- `source` can be relative (`:luafile .nvim.lua`), so make the directory
-- absolute before it is used as a prefix.
local source = debug.getinfo(1, "S").source:sub(2)
local root = vim.fs.normalize(vim.fn.fnamemodify(source, ":p:h"))

-- A fixed group name plus `clear = true` would let a second bureau repository
-- delete the autocmds of the first one in the same session.
local group_name = "BureauNotes_" .. vim.fn.sha256(root):sub(1, 16)
local group = vim.api.nvim_create_augroup(group_name, { clear = true })

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

-- Whether `path` is `directory` or sits below it. Both are normalized, and
-- Windows compares case-insensitively.
local function under(path, directory)
  path = vim.fs.normalize(path)
  directory = vim.fs.normalize(directory)
  if vim.fn.has("win32") == 1 then
    path, directory = path:lower(), directory:lower()
  end

  local prefix = directory
  if prefix:sub(-1) ~= "/" then
    prefix = prefix .. "/"
  end
  return path == directory or path:sub(1, #prefix) == prefix
end

-- The fence `line` opens or closes, matching `src/fence.rs`: at most three
-- spaces of indentation, then three or more backticks or tildes.
local function fence_run(line)
  local indent = #(line:match("^( *)") or "")
  if indent > 3 then
    return nil
  end

  local rest = line:sub(indent + 1)
  local character = rest:sub(1, 1)
  if character ~= "`" and character ~= "~" then
    return nil
  end

  local run = rest:match("^" .. character .. "+")
  if not run or #run < 3 then
    return nil
  end

  return character, #run, rest:sub(#run + 1)
end

-- One fold level per line: headings set the base level, indentation adds one
-- level per four columns. Fenced code blocks are kept out of the calculation.
local function fold_levels(buffer)
  local lines = vim.api.nvim_buf_get_lines(buffer, 0, -1, false)
  local levels = {}
  local heading = 1
  local fence_character = nil
  local fence_length = 0

  for index, line in ipairs(lines) do
    local character, length, after = fence_run(line)

    if fence_character then
      -- A closing fence uses the same character, is at least as long as the
      -- opening one, and has nothing but whitespace after it.
      if
        character == fence_character
        and length >= fence_length
        and after:match("^%s*$")
      then
        fence_character, fence_length = nil, 0
      end
      levels[index] = tostring(heading)
    elseif character then
      fence_character, fence_length = character, length
      levels[index] = tostring(heading)
    else
      local hashes = line:match("^(#+)%s")
      if hashes then
        heading = #hashes
        levels[index] = ">" .. heading
      else
        local whitespace = line:match("^%s*") or ""
        levels[index] = tostring(math.min(heading + math.floor(columns(whitespace) / 4), 20))
      end
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
  return name ~= "" and under(name, root)
end

local function apply()
  local buffer = vim.api.nvim_get_current_buf()
  if vim.bo[buffer].filetype ~= "markdown" or not in_repository(buffer) then
    return
  end

  -- Remember the window's own fold settings so leaving a bureau note can put
  -- them back. `root` says which repository currently owns those settings.
  if not vim.w.bureau_notes_saved then
    vim.w.bureau_notes_saved = {
      foldmethod = vim.wo.foldmethod,
      foldexpr = vim.wo.foldexpr,
      foldlevel = vim.wo.foldlevel,
      root = root,
    }
  else
    vim.w.bureau_notes_saved.root = root
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
  local buffer = vim.api.nvim_get_current_buf()
  local saved = vim.w.bureau_notes_saved
  if not saved then
    return
  end

  -- Keep the settings for a Markdown buffer in the repository that owns them.
  if vim.bo[buffer].filetype == "markdown" and saved.root then
    local name = vim.api.nvim_buf_get_name(buffer)
    if name ~= "" and under(name, saved.root) then
      return
    end
  end

  vim.wo.foldmethod = saved.foldmethod
  vim.wo.foldexpr = saved.foldexpr
  vim.wo.foldlevel = saved.foldlevel
  vim.w.bureau_notes_saved = nil
end

vim.api.nvim_create_autocmd({ "BufEnter", "FileType" }, {
  group = group,
  callback = function()
    restore()
    apply()
  end,
})

apply()
