// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2023 Stacks Open Internet Foundation
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

use super::chainstate::{StacksAddress, VRFSeed};
use crate::deps_common::bitcoin::util::hash::Sha256dHash;
use crate::types::chainstate::{
    BlockHeaderHash, BurnchainHeaderHash, ConsensusHash, SortitionId, StacksBlockId, TrieHash,
};
use crate::util::hash::{Hash160, Sha512Trunc256Sum};
use crate::util::secp256k1::MessageSignature;
use crate::util::vrf::VRFProof;

pub const NO_PARAMS: &[&dyn ToSql] = &[];

impl FromSql for Sha256dHash {
    fn column_result(value: ValueRef) -> FromSqlResult<Sha256dHash> {
        match value {
            ValueRef::Blob(bytes) => Ok(Sha256dHash::from(bytes)),
            ValueRef::Text(hex_str) => {
                let hex_str =
                    std::str::from_utf8(hex_str).map_err(|_e| FromSqlError::InvalidType)?;
                Sha256dHash::from_hex(hex_str).map_err(|_e| FromSqlError::InvalidType)
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

impl ToSql for Sha256dHash {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.to_bytes().to_vec().into())
    }
}

impl ToSql for StacksAddress {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let addr_str = self.to_string();
        Ok(addr_str.into())
    }
}

// Implement rusqlite traits for a bunch of structs that used to be defined
//  in the chainstate code
impl_byte_array_rusqlite_only!(ConsensusHash);
impl_byte_array_rusqlite_only!(Hash160);
impl_byte_array_rusqlite_only!(BlockHeaderHash);
impl_byte_array_rusqlite_only!(VRFSeed);
impl_byte_array_rusqlite_only!(BurnchainHeaderHash);
impl_byte_array_rusqlite_only!(VRFProof);
impl_byte_array_rusqlite_only!(TrieHash);
impl_byte_array_rusqlite_only!(Sha512Trunc256Sum);
impl_byte_array_rusqlite_only!(MessageSignature);
impl_byte_array_rusqlite_only!(SortitionId);
impl_byte_array_rusqlite_only!(StacksBlockId);

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use super::*;

    #[test]
    fn typed_hashes_roundtrip_as_blobs_and_match_in_where_queries() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE hashes (block_hash BLOB PRIMARY KEY)",
            NO_PARAMS,
        )
        .unwrap();

        let block_hash = BlockHeaderHash::from_hex(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO hashes (block_hash) VALUES (?1)",
            params![&block_hash],
        )
        .unwrap();

        let stored_type: String = conn
            .query_row("SELECT typeof(block_hash) FROM hashes", NO_PARAMS, |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stored_type, "blob");

        let loaded: BlockHeaderHash = conn
            .query_row(
                "SELECT block_hash FROM hashes WHERE block_hash = ?1",
                params![&block_hash],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(loaded, block_hash);
    }

    #[test]
    fn typed_hashes_read_legacy_text_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE hashes (block_hash TEXT PRIMARY KEY)",
            NO_PARAMS,
        )
        .unwrap();

        let block_hash_hex = "89abcdef0123456789abcdef0123456789abcdef0123456789abcdef01234567";
        let expected = BlockHeaderHash::from_hex(block_hash_hex).unwrap();

        conn.execute(
            "INSERT INTO hashes (block_hash) VALUES (?1)",
            params![block_hash_hex],
        )
        .unwrap();

        let loaded: BlockHeaderHash = conn
            .query_row("SELECT block_hash FROM hashes", NO_PARAMS, |row| row.get(0))
            .unwrap();
        assert_eq!(loaded, expected);
    }

    #[test]
    fn sha256d_hashes_roundtrip_as_blobs() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE hashes (hash BLOB PRIMARY KEY)", NO_PARAMS)
            .unwrap();

        let hash = Sha256dHash::from_hex(
            "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        )
        .unwrap();

        conn.execute("INSERT INTO hashes (hash) VALUES (?1)", params![&hash])
            .unwrap();

        let stored_type: String = conn
            .query_row("SELECT typeof(hash) FROM hashes", NO_PARAMS, |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stored_type, "blob");

        let loaded: Sha256dHash = conn
            .query_row(
                "SELECT hash FROM hashes WHERE hash = ?1",
                params![&hash],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(loaded, hash);
    }

    #[test]
    fn blob_hash_order_matches_hex_lexicographic_order() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE hashes (block_hash BLOB PRIMARY KEY)",
            NO_PARAMS,
        )
        .unwrap();

        let hashes = [
            BlockHeaderHash::from_hex(
                "000000000000000000000000000000000000000000000000000000000000000f",
            )
            .unwrap(),
            BlockHeaderHash::from_hex(
                "0000000000000000000000000000000000000000000000000000000000000010",
            )
            .unwrap(),
            BlockHeaderHash::from_hex(
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            )
            .unwrap(),
        ];

        for hash in &hashes {
            conn.execute("INSERT INTO hashes (block_hash) VALUES (?1)", params![hash])
                .unwrap();
        }

        let ordered: Vec<BlockHeaderHash> = conn
            .prepare("SELECT block_hash FROM hashes ORDER BY block_hash ASC")
            .unwrap()
            .query_map(NO_PARAMS, |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        let mut expected = hashes.to_vec();
        expected.sort_by_key(|hash| hash.to_hex());

        assert_eq!(ordered, expected);
    }
}
