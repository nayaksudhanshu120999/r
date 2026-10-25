fn match_notes(&self, raw_query: &str, q_lower: &str, out: &mut Vec<CandidateItem>) {
        for note in &self.config.notes {
            // Direct inline `<bang> <content>` fallback if chip wasn't extracted
            let prefix = format!("{} ", note.bang);
            if q_lower.starts_with(&prefix) {
                let remainder = raw_query[prefix.len()..].trim();
                if !remainder.is_empty() {
                    let file_path = resolve_note_file_path(&note.file_path);
                    out.push(CandidateItem {
                        title: format!("Save to {}: \"{}\"", note.file_path, remainder),
                        subtitle: format!(
                            "Appends separator + date & time + note to {}",
                            file_path.display()
                        ),
                        badge: format!("!{}", note.bang),
                        icon_override: Some("💾".to_string()),
                        category: ShelfCategory::Notes,
                        can_elevate: false,
                        score: 3_050,
                        target: ExecutionTarget::NoteAction(NoteActionKind::AppendLine {
                            file_path,
                            content: remainder.to_string(),
                        }),
                    });
                    continue;
                }
            }

            let name_lower = note.name.to_ascii_lowercase();
            let file_lower = note.file_path.to_ascii_lowercase();
            let s1 = self.compute_smart_score(q_lower, &note.bang);
            let s2 = self.compute_smart_score(q_lower, &name_lower);
            let s3 = self.compute_smart_score(q_lower, &file_lower);

            if let Some(score) = [s1, s2, s3].into_iter().flatten().max() {
                if q_lower == note.bang {
                    continue;
                }
                out.push(CandidateItem {
                    title: format!("{} (!{})", note.name, note.bang),
                    subtitle: format!(
                        "Note file: {} • Press Enter to write or view notes",
                        note.file_path
                    ),
                    badge: format!("!{}", note.bang),
                    icon_override: Some("📝".to_string()),
                    category: ShelfCategory::Notes,
                    can_elevate: false,
                    score,
                    target: ExecutionTarget::ActivateBang {
                        bang: note.bang.clone(),
                        label: format!("📝 {}", note.bang.to_ascii_uppercase()),
                    },
                });
            }
        }
    }
