#!/usr/bin/env ruby
# frozen_string_literal: true

require 'fileutils'
require 'json'
require 'pathname'

react_native_root, pods_dir, output_dir = ARGV
abort 'usage: pod-header-shim.rb <react-native-root> <Pods-dir> <output-dir>' unless output_dir

react_native_root = File.expand_path(react_native_root)
pods_dir = File.expand_path(pods_dir)
output_dir = File.expand_path(output_dir)
local_specs_dir = File.join(pods_dir, 'Local Podspecs')

abort "missing CocoaPods evaluated podspec directory: #{local_specs_dir}" unless Dir.exist?(local_specs_dir)

HEADER_EXTENSIONS = %w[.h .hh .hpp .hxx].freeze

def values_for(spec, key)
  values = []
  values.concat(Array(spec[key])) if spec.key?(key)
  ios = spec['ios']
  values.concat(Array(ios[key])) if ios.is_a?(Hash) && ios.key?(key)
  values.select { |value| value.is_a?(String) && !value.empty? }
end

def expand_patterns(source_root, patterns)
  patterns.flat_map do |pattern|
    Dir.glob(File.expand_path(pattern, source_root), File::FNM_EXTGLOB)
  end.select { |path| File.file?(path) }.map { |path| File.expand_path(path) }.uniq
end

def header_file?(path)
  HEADER_EXTENSIONS.include?(File.extname(path).downcase)
end

def inside?(path, directory)
  path = Pathname.new(File.expand_path(path))
  directory = Pathname.new(File.expand_path(directory))
  path.ascend.any? { |ancestor| ancestor == directory }
end

def link_header(include_root, namespace, source_root, mappings_dir, header)
  relative = if mappings_dir
               mappings_root = File.expand_path(mappings_dir, source_root)
               if inside?(header, mappings_root)
                 Pathname.new(header).relative_path_from(Pathname.new(mappings_root)).to_s
               else
                 File.basename(header)
               end
             else
               File.basename(header)
             end

  destination = File.join(include_root, namespace, relative)
  FileUtils.mkdir_p(File.dirname(destination))

  if File.exist?(destination) || File.symlink?(destination)
    existing = File.realpath(destination) rescue nil
    return if existing == File.realpath(header)

    abort <<~MSG
      ambiguous CocoaPods public header mapping: #{destination}
        first: #{existing || destination}
        next:  #{header}
    MSG
  end

  File.symlink(File.expand_path(header), destination)
end

def process_spec(spec, source_root, include_root, root_name, inherited_header_dir = nil, inherited_mappings_dir = nil)
  header_dir = spec.key?('header_dir') ? spec['header_dir'] : inherited_header_dir
  mappings_dir = spec.key?('header_mappings_dir') ? spec['header_mappings_dir'] : inherited_mappings_dir
  namespace = header_dir.to_s.empty? ? root_name : header_dir

  public_patterns = values_for(spec, 'public_header_files')
  source_patterns = values_for(spec, 'source_files')
  header_candidates = if public_patterns.empty?
                        expand_patterns(source_root, source_patterns).select { |path| header_file?(path) }
                      else
                        expand_patterns(source_root, public_patterns).select { |path| header_file?(path) }
                      end

  hidden = %w[private_header_files project_header_files exclude_files].flat_map do |key|
    expand_patterns(source_root, values_for(spec, key))
  end.to_h { |path| [path, true] }

  header_candidates.reject { |path| hidden[path] }.each do |header|
    link_header(include_root, namespace, source_root, mappings_dir, header)
  end

  Array(spec['subspecs']).each do |subspec|
    next unless subspec.is_a?(Hash)

    process_spec(subspec, source_root, include_root, root_name, header_dir, mappings_dir)
  end
end

podspec_paths = Dir.glob(File.join(react_native_root, '**', '*.podspec')).sort
podspec_by_name = Hash.new { |hash, key| hash[key] = [] }
podspec_paths.each do |path|
  podspec_by_name[File.basename(path, '.podspec')] << path
end

FileUtils.rm_rf(output_dir)
FileUtils.mkdir_p(output_dir)
include_roots = []
processed_specs = 0

Dir.glob(File.join(local_specs_dir, '*.podspec.json')).sort.each do |json_path|
  spec = JSON.parse(File.read(json_path))
  root_name = spec['name'].to_s.split('/').first
  next if root_name.empty?

  candidates = podspec_by_name[root_name]
  next if candidates.empty?

  # React Native podspec names are unique inside one installed package. Prefer the
  # shallowest matching file if a package happens to carry historical duplicates.
  podspec_path = candidates.min_by { |path| Pathname.new(path).relative_path_from(Pathname.new(react_native_root)).each_filename.count }
  source_root = File.dirname(podspec_path)
  safe_name = root_name.gsub(/[^A-Za-z0-9_.-]/, '_')
  include_root = File.join(output_dir, safe_name)

  process_spec(spec, source_root, include_root, root_name)
  next unless Dir.exist?(include_root) && !Dir.empty?(include_root)

  include_roots << include_root
  processed_specs += 1
end

bridge_roots, other_roots = include_roots.uniq.partition do |root|
  File.file?(File.join(root, 'React', 'RCTBridgeModule.h')) || File.symlink?(File.join(root, 'React', 'RCTBridgeModule.h'))
end

if bridge_roots.empty?
  abort <<~MSG
    unable to reconstruct React/RCTBridgeModule.h from CocoaPods evaluated podspecs
      React Native: #{react_native_root}
      podspecs:     #{local_specs_dir}
  MSG
end

warn "Fresnica React Native source headers: #{processed_specs} evaluated CocoaPods podspecs"
(bridge_roots + other_roots).each { |root| puts root }
