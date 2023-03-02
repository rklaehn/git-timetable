use anyhow::{bail, Result};
use clap::Parser;
use git2::{BranchType, Commit, Repository};
use iter_tools::Itertools;
use std::fmt::Debug;
use std::ops::Range;

fn list_commits(repo_path: String, time_range: &Range<i64>, author: &Option<String>) -> Result<Vec<RepoAndCommit>> {
    let repo = Repository::open(&repo_path)?;
    let branches = repo.branches(Some(BranchType::Local))?;
    let mut commits = Vec::new();

    for branch in branches {
        let (branch, _) = branch?;

        let branch_name = branch.name()?.unwrap_or("No branch").to_string();
        let branch_oid = branch.get().peel_to_commit()?.id();

        let mut revwalk = repo.revwalk()?;
        revwalk.push(branch_oid)?;

        for oid in revwalk {
            let oid = oid?;
            let commit = repo.find_commit(oid)?;
            let date = commit.time().seconds();

            if date < time_range.start || date > time_range.end {
                continue;
            }

            let repo_and_commit =
                RepoAndCommit::new(repo_path.clone(), branch_name.clone(), commit);

            if let Some(author) = author {
                if !repo_and_commit.author.contains(author) {
                    continue;
                }
            }

            commits.push(repo_and_commit);
        }
    }

    Ok(commits)
}

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    repositories: Vec<String>,

    #[arg(short, long)]
    since: Option<String>,

    #[arg(short, long)]
    until: Option<String>,

    #[arg(short, long)]
    format: Option<String>,

    #[arg(short, long)]
    author: Option<String>,
}

#[derive(Debug)]
struct RepoAndCommit {
    message: String,
    summary: String,
    author: String,
    commit: String,
    branch: String,
    repo: String,
    date: i64,
}

impl RepoAndCommit {
    fn new<'a>(repo: String, branch: String, commit: Commit<'a>) -> Self {
        Self {
            summary: commit.summary().unwrap_or("No summary").to_string(),
            message: commit.message().unwrap_or("No message").to_string(),
            author: commit.author().to_string(),
            commit: commit.id().to_string(),
            date: commit.time().seconds(),
            repo,
            branch,
        }
    }

    fn date(&self) -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::from_timestamp_opt(self.date, 0).unwrap()
    }
}

fn parse_lenient(s: &str) -> Result<i64> {
    let date = chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.timestamp())
        .or_else(|_| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().timestamp())
        })?;

    Ok(date)
}

fn parse_time_range(since: Option<&str>, until: Option<&str>) -> Result<Range<i64>> {
    let since = match since {
        Some(date) => parse_lenient(date),
        None => Ok(0),
    }?;

    let until = match until {
        Some(date) => parse_lenient(date),
        None => Ok(i64::MAX),
    }?;

    Ok(since..until)
}

fn main() -> Result<()> {
    let opts: Args = Args::parse();

    let since = opts.since.as_deref();
    let until = opts.until.as_deref();
    let time_range = parse_time_range(since, until)?;
    let repositories = opts.repositories;
    let format = opts.format.map(|x| x).unwrap_or("flat".to_string());
    let author = opts.author;

    let mut commits = Vec::new();
    for repo_path in repositories {
        commits.extend(list_commits(repo_path, &time_range, &author)?);
    }
    commits.sort_by_key(|c| c.date);
    match format.as_str() {
        "flat" => {
            for commit in commits {
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    commit.date(),
                    commit.repo,
                    commit.branch,
                    commit.commit,
                    commit.summary,
                    commit.author,
                );
            }
        }
        "daily" => {
            commits
                .into_iter()
                .group_by(|x| x.date().date())
                .into_iter()
                .for_each(|(date, commits)| {
                    println!("{}", date);
                    for commit in commits {
                        let time = commit.date().time();
                        println!(
                            "\t\t{}\t{}\t{}\t{}\t{}",
                            time, commit.repo, commit.branch, commit.summary, commit.author
                        );
                    }
                });
        }
        _ => {
            bail!("unknown format: {}", format);
        }
    }

    Ok(())
}
