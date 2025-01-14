use super::{SlashCommand, SlashCommandOutput};
use crate::prompt_library::PromptStore;
use anyhow::{anyhow, Context, Result};
use assistant_slash_command::{ArgumentCompletion, SlashCommandOutputSection};
use gpui::{Task, WeakView};
use language::LspAdapterDelegate;
use std::sync::{atomic::AtomicBool, Arc};
use ui::prelude::*;
use workspace::Workspace;

pub(crate) struct PromptSlashCommand;

impl SlashCommand for PromptSlashCommand {
    fn name(&self) -> String {
        "prompt".into()
    }

    fn description(&self) -> String {
        "insert prompt from library".into()
    }

    fn menu_text(&self) -> String {
        "Insert Prompt from Library".into()
    }

    fn requires_argument(&self) -> bool {
        true
    }

    fn complete_argument(
        self: Arc<Self>,
        arguments: &[String],
        _cancellation_flag: Arc<AtomicBool>,
        _workspace: Option<WeakView<Workspace>>,
        cx: &mut WindowContext,
    ) -> Task<Result<Vec<ArgumentCompletion>>> {
        let store = PromptStore::global(cx);
        let query = arguments.last().cloned().unwrap_or_default();
        cx.background_executor().spawn(async move {
            let prompts = store.await?.search(query).await;
            Ok(prompts
                .into_iter()
                .filter_map(|prompt| {
                    let prompt_title = prompt.title?.to_string();
                    Some(ArgumentCompletion {
                        label: prompt_title.clone().into(),
                        new_text: prompt_title,
                        run_command: true,
                    })
                })
                .collect())
        })
    }

    fn run(
        self: Arc<Self>,
        arguments: &[String],
        _workspace: WeakView<Workspace>,
        _delegate: Option<Arc<dyn LspAdapterDelegate>>,
        cx: &mut WindowContext,
    ) -> Task<Result<SlashCommandOutput>> {
        let Some(title) = arguments.first() else {
            return Task::ready(Err(anyhow!("missing prompt name")));
        };

        let store = PromptStore::global(cx);
        let title = SharedString::from(title.to_string());
        let prompt = cx.background_executor().spawn({
            let title = title.clone();
            async move {
                let store = store.await?;
                let prompt_id = store
                    .id_for_title(&title)
                    .with_context(|| format!("no prompt found with title {:?}", title))?;
                let body = store.load(prompt_id).await?;
                anyhow::Ok(body)
            }
        });
        cx.foreground_executor().spawn(async move {
            let mut prompt = prompt.await?;

            if prompt.starts_with('/') {
                // Prevent an edge case where the inserted prompt starts with a slash command (that leads to funky rendering).
                prompt.insert(0, '\n');
            }
            if prompt.is_empty() {
                prompt.push('\n');
            }
            let range = 0..prompt.len();
            Ok(SlashCommandOutput {
                text: prompt,
                sections: vec![SlashCommandOutputSection {
                    range,
                    icon: IconName::Library,
                    label: title,
                }],
                run_commands_in_text: true,
            })
        })
    }
}
