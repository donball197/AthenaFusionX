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


<script>
</script>

<div class="fixed inset-0 pointer-events-none z-0 overflow-hidden flex items-center justify-center opacity-10">
    <div class="relative w-[820px] h-[820px] animate-[spin_28s_linear_infinite]">
        <div class="absolute inset-0 border border-cyan-400/30 rounded-full border-dashed"></div>
        <div class="absolute inset-12 border border-cyan-500/20 rounded-full"></div>
        <div class="absolute inset-28 border border-cyan-600/10 rounded-full"></div>
    </div>
    
    <!-- Corner brackets -->
    <div class="absolute top-8 left-8 w-8 h-8 border-l border-t border-cyan-400/60"></div>
    <div class="absolute top-8 right-8 w-8 h-8 border-r border-t border-cyan-400/60"></div>
    <div class="absolute bottom-8 left-8 w-8 h-8 border-l border-b border-cyan-400/60"></div>
    <div class="absolute bottom-8 right-8 w-8 h-8 border-r border-b border-cyan-400/60"></div>
</div>

<style>
    @keyframes spin {
        from { transform: rotate(0deg); }
        to { transform: rotate(360deg); }
    }
</style>
