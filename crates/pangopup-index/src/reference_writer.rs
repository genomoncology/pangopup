//! The single byte-producing `PGRREF01` writer.

use crate::reference_wire::{
    AmbiguityRun, CONTIG_COUNT, DENSE_OFFSET, DIRECTORY_BYTES, DIRECTORY_ENTRY_BYTES,
    DirectoryEntry, ENCODING, HEADER_BYTES, MAGIC, MAX_AMBIGUITY_RUNS, MAX_MEMBER_BYTES, RUN_BYTES,
    ReferenceContigPlan, ReferenceIndexError, VERSION, align8, iupac_code,
};
use pangopup_core::Grch38Contig;
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub struct ReferenceMemberWriter {
    file: File,
    entries: [DirectoryEntry; CONTIG_COUNT],
    seen: [bool; CONTIG_COUNT],
    run_scratch: File,
    run_scratch_path: PathBuf,
    run_counts: [u64; CONTIG_COUNT],
    total_runs: u64,
    active: Option<ActiveWrite>,
}

struct ActiveWrite {
    code: u8,
    expected: u64,
    position: u64,
    packed: u8,
    in_byte: u8,
    current_run: Option<AmbiguityRun>,
}

impl ReferenceMemberWriter {
    pub fn create(
        path: &Path,
        plans: &[ReferenceContigPlan; CONTIG_COUNT],
    ) -> Result<Self, ReferenceIndexError> {
        let mut expected_dense = DENSE_OFFSET;
        let mut entries = [DirectoryEntry {
            code: 0,
            bases: 0,
            dense_offset: 0,
            dense_length: 0,
            run_offset: 0,
            run_count: 0,
        }; CONTIG_COUNT];
        for (index, (entry, plan)) in entries.iter_mut().zip(plans).enumerate() {
            if plan.contig.code() != (index + 1) as u8
                || plan.bases == 0
                || plan.bases > u32::MAX as u64
            {
                return Err(ReferenceIndexError::Corrupt("writer plan"));
            }
            let dense_length = plan.bases.div_ceil(4);
            *entry = DirectoryEntry {
                code: plan.contig.code(),
                bases: plan.bases,
                dense_offset: expected_dense,
                dense_length,
                run_offset: 0,
                run_count: 0,
            };
            expected_dense = expected_dense
                .checked_add(dense_length)
                .ok_or(ReferenceIndexError::Corrupt("writer arithmetic"))?;
        }
        let ambiguity_offset = align8(expected_dense)?;
        if ambiguity_offset > MAX_MEMBER_BYTES {
            return Err(ReferenceIndexError::Corrupt("writer member size"));
        }
        let file = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        file.set_len(ambiguity_offset)?;
        let run_scratch_path = path.with_extension("ambiguity.scratch");
        let run_scratch = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&run_scratch_path)?;
        Ok(Self {
            file,
            entries,
            seen: [false; CONTIG_COUNT],
            run_scratch,
            run_scratch_path,
            run_counts: [0; CONTIG_COUNT],
            total_runs: 0,
            active: None,
        })
    }

    pub fn write_contig(
        &mut self,
        contig: Grch38Contig,
        sequence: &[u8],
    ) -> Result<(), ReferenceIndexError> {
        self.begin_contig(contig)?;
        self.write_bases(sequence)?;
        self.finish_contig()
    }

    pub fn begin_contig(&mut self, contig: Grch38Contig) -> Result<(), ReferenceIndexError> {
        let index = (contig.code() - 1) as usize;
        let entry = self.entries[index];
        if self.active.is_some() || self.seen[index] {
            return Err(ReferenceIndexError::Corrupt("writer contig state"));
        }
        self.file.seek(SeekFrom::Start(entry.dense_offset))?;
        self.active = Some(ActiveWrite {
            code: contig.code(),
            expected: entry.bases,
            position: 0,
            packed: 0,
            in_byte: 0,
            current_run: None,
        });
        Ok(())
    }

    pub fn write_bases(&mut self, sequence: &[u8]) -> Result<(), ReferenceIndexError> {
        let mut active = self
            .active
            .take()
            .ok_or(ReferenceIndexError::Corrupt("writer contig state"))?;
        for &original in sequence {
            if active.position >= active.expected {
                return Err(ReferenceIndexError::Corrupt("writer sequence length"));
            }
            let byte = original.to_ascii_uppercase();
            let code = iupac_code(byte).ok_or(ReferenceIndexError::Corrupt("writer IUPAC"))?;
            let dense = if code < 4 { code } else { 0 };
            active.packed |= dense << (active.in_byte * 2);
            active.in_byte += 1;
            if active.in_byte == 4 {
                self.file.write_all(&[active.packed])?;
                active.packed = 0;
                active.in_byte = 0;
            }
            if code >= 4 {
                let position = u32::try_from(active.position)
                    .map_err(|_| ReferenceIndexError::Corrupt("writer position"))?;
                match active.current_run.as_mut() {
                    Some(run) if run.code == code && run.start + run.length == position => {
                        run.length = run
                            .length
                            .checked_add(1)
                            .ok_or(ReferenceIndexError::Corrupt("writer run"))?;
                    }
                    Some(run) => {
                        self.append_run(active.code, *run)?;
                        *run = AmbiguityRun {
                            start: position,
                            length: 1,
                            code,
                        };
                    }
                    None => {
                        active.current_run = Some(AmbiguityRun {
                            start: position,
                            length: 1,
                            code,
                        })
                    }
                }
            } else if let Some(run) = active.current_run.take() {
                self.append_run(active.code, run)?;
            }
            active.position += 1;
        }
        self.active = Some(active);
        Ok(())
    }

    pub fn finish_contig(&mut self) -> Result<(), ReferenceIndexError> {
        let mut active = self
            .active
            .take()
            .ok_or(ReferenceIndexError::Corrupt("writer contig state"))?;
        if active.position != active.expected {
            return Err(ReferenceIndexError::Corrupt("writer sequence length"));
        }
        if active.in_byte != 0 {
            self.file.write_all(&[active.packed])?;
        }
        if let Some(run) = active.current_run.take() {
            self.append_run(active.code, run)?;
        }
        self.seen[(active.code - 1) as usize] = true;
        Ok(())
    }

    fn append_run(
        &mut self,
        contig_code: u8,
        run: AmbiguityRun,
    ) -> Result<(), ReferenceIndexError> {
        if self.total_runs >= MAX_AMBIGUITY_RUNS {
            return Err(ReferenceIndexError::Corrupt("writer run limit"));
        }
        let mut bytes = [0_u8; RUN_BYTES as usize];
        bytes[0] = contig_code;
        bytes[1..5].copy_from_slice(&run.start.to_le_bytes());
        bytes[5..9].copy_from_slice(&run.length.to_le_bytes());
        bytes[9] = run.code;
        self.run_scratch.write_all(&bytes)?;
        self.run_counts[(contig_code - 1) as usize] += 1;
        self.total_runs += 1;
        Ok(())
    }

    pub fn finish(mut self) -> Result<[u64; CONTIG_COUNT], ReferenceIndexError> {
        if self.active.is_some() || self.seen.iter().any(|seen| !seen) {
            return Err(ReferenceIndexError::Corrupt("writer missing contig"));
        }
        let dense_end = self.entries[24].dense_offset + self.entries[24].dense_length;
        let ambiguity_offset = align8(dense_end)?;
        self.file.seek(SeekFrom::Start(ambiguity_offset))?;
        self.run_scratch.flush()?;
        let mut scratch_record = [0_u8; RUN_BYTES as usize];
        for wanted_code in 1_u8..=25 {
            self.run_scratch.seek(SeekFrom::Start(0))?;
            for _ in 0..self.total_runs {
                self.run_scratch.read_exact(&mut scratch_record)?;
                if scratch_record[0] == wanted_code {
                    self.file.write_all(&scratch_record[1..5])?;
                    self.file.write_all(&scratch_record[5..9])?;
                    self.file.write_all(&[scratch_record[9]])?;
                    self.file.write_all(&[0; 7])?;
                }
            }
        }
        let file_length = ambiguity_offset + self.total_runs * RUN_BYTES;
        if file_length > MAX_MEMBER_BYTES {
            return Err(ReferenceIndexError::Corrupt("writer member size"));
        }
        self.file.set_len(file_length)?;
        let mut prior = 0_u64;
        for (entry, count) in self.entries.iter_mut().zip(self.run_counts) {
            entry.run_count = count;
            entry.run_offset = if count == 0 {
                0
            } else {
                ambiguity_offset + prior * RUN_BYTES
            };
            prior += count;
        }
        self.file.seek(SeekFrom::Start(0))?;
        let mut header = [0_u8; HEADER_BYTES];
        header[0..8].copy_from_slice(MAGIC);
        header[8..10].copy_from_slice(&VERSION.to_le_bytes());
        header[10] = ENCODING;
        header[11] = CONTIG_COUNT as u8;
        header[16..24].copy_from_slice(&file_length.to_le_bytes());
        header[24..32].copy_from_slice(&(HEADER_BYTES as u64).to_le_bytes());
        header[32..40].copy_from_slice(&(DIRECTORY_BYTES as u64).to_le_bytes());
        header[40..48].copy_from_slice(&DENSE_OFFSET.to_le_bytes());
        header[48..56].copy_from_slice(&ambiguity_offset.to_le_bytes());
        header[56..64].copy_from_slice(&self.total_runs.to_le_bytes());
        self.file.write_all(&header)?;
        for entry in self.entries {
            let mut bytes = [0_u8; DIRECTORY_ENTRY_BYTES];
            bytes[0] = entry.code;
            bytes[8..16].copy_from_slice(&entry.bases.to_le_bytes());
            bytes[16..24].copy_from_slice(&entry.dense_offset.to_le_bytes());
            bytes[24..32].copy_from_slice(&entry.dense_length.to_le_bytes());
            bytes[32..40].copy_from_slice(&entry.run_offset.to_le_bytes());
            bytes[40..48].copy_from_slice(&entry.run_count.to_le_bytes());
            self.file.write_all(&bytes)?;
        }
        self.file.sync_all()?;
        fs::remove_file(&self.run_scratch_path)?;
        Ok(self.run_counts)
    }
}

impl Drop for ReferenceMemberWriter {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.run_scratch_path);
    }
}
