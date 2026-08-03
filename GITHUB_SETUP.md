# GitHub pe kaise daalein — step by step

Windows + PowerShell ke liye. Har command copy-paste karo, ek ek karke.

---

## Step 1 — Package extract karo

Jo `.tar.gz` file download ki hai, use extract karo:

```powershell
cd ~\Desktop
tar -xzf semver-rs.tar.gz
cd semver-rs
```

Check karo sab aa gaya:

```powershell
dir
```

`src`, `tests`, `scripts`, `Cargo.toml`, `README.md` dikhne chahiye.

---

## Step 2 — Chalake dekho ki kaam kar raha hai

```powershell
cargo build --release
cargo test
```

Sab green hona chahiye. Agar nahi, aage mat badho — pehle wo theek karo.

---

## Step 3 — GitHub pe naya repo banao

1. https://github.com/new pe jao
2. Repository name: `semver-rs`
3. **Public** rakho
4. **README, .gitignore, license kuch mat add karo** — already hain
5. "Create repository" dabao

---

## Step 4 — Push karo

GitHub page pe jo commands dikhengi, unme se ye chalao (apna username daalke):

```powershell
git init
git add .
git commit -m "SemVer port: JS to Rust, verified against the original"
git branch -M main
git remote add origin https://github.com/codewitharyan29/semver-rs.git
git push -u origin main
```

Agar password maange, to GitHub password nahi chalega — **Personal Access Token** chahiye:
- https://github.com/settings/tokens pe jao
- "Generate new token (classic)"
- `repo` scope tick karo
- Token copy karke password ki jagah paste karo

---

## Step 5 — Link check karo

Browser me kholo:

```
https://github.com/codewitharyan29/semver-rs
```

README dikhna chahiye with the numbers.

**Yehi link submit karna hai.**

---

## Agar kuch fasta hai

**"git: command not found"** → Git install karo: https://git-scm.com/download/win

**"cargo: command not found"** → Rust install karo: https://rustup.rs

**"failed to push"** → shayad repo me pehle se kuch hai:
```powershell
git pull origin main --allow-unrelated-histories
git push -u origin main
```

**"Permission denied"** → token galat hai, naya banao (Step 4 dekho)
