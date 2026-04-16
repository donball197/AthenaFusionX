/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


#[cfg(test)]
mod tests {
    use nexus_singularity::hiram_core::Hiram;

    #[test]
    fn test_hiram_initialization() {
        let hiram = Hiram::new(3600);

        // It should start at 100% integrity
        assert_eq!(hiram.integrity, 100);
        assert!(hiram.last_verified.is_none());
        assert_eq!(hiram.verification_interval, 3600);
    }

    #[test]
    fn test_hiram_verify_execution() {
        let mut hiram = Hiram::new(3600);

        // Run the verify function. It shouldn't panic, and it should stamp the time.
        hiram.verify();
        assert!(hiram.last_verified.is_some());
    }
}
