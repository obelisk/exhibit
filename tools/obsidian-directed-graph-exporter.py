"""
Usage

python -m venv venv
source venv/bin/activate
pip install -r requirements.txt
python obsidian-directed-graph-exporter.py {{path_to_markdown_files}} {{path_to_output_images}} {{first_slide_name}} {{gen_images true|false}}
"""

import os
import re
import json
import argparse
from typing import List, Optional, Tuple
from PIL import Image, ImageDraw, ImageFont

FONT_PATH = "./assets/fonts/Open_Sans/OpenSans-Italic-VariableFont_wdth,wght.ttf"
# FONT_PATH = "./assets/fonts/AppleColorEmoji.ttf"
SLIDE_BACKGROUND_IMAGE_PATH = "./assets/background_grad.png"
CONTAINS_PATH_POLL = "CONTAINS_PATH_POLL"
STATE_POLL = "STATE_POLL"


class Poll:
    def __init__(self, name: str, poll_type: str, options: List[Tuple[str, str]]): 
        self.name = name
        self.poll_type = poll_type
        self.options = options   # List (Choice text, Destination slide title)
        
    def to_json(self, slides_map):
        # With slides_map, resolve slide titles to index
        resolved_slide_indices = []
        for opt in self.options:
            found_slide = slides_map.get(opt[1])
            if found_slide is None:
                print(f"Error can not resolve next slide for current poll {self.name}")
                return
            resolved_slide_indices.append(found_slide.index)

        fields = {
            'slide_advancement_from_poll_results': {
                'poll_name': self.name,
                'results_to_slide_number': resolved_slide_indices
            }  
        }
        # Extend with poll render fields if not state branching poll
        if self.poll_type == CONTAINS_PATH_POLL:
            fields.update({
                'poll': {
                    'name': self.name if self.name is not None else "NO NAME",
                    'options': [opt[0] for opt in self.options],
                    'vote_type': {
                        'SingleBinary': {
                            'choice': ''
                        }
                    }
                },
                'poll_render': {
                    'refresh_interval': 3,
                    'type_': 'centroid',
                    'x': 68,
                    'y': 2,
                    'scale': 30,
                    'vbx': 230,
                    'vby': 230
                }
            })
        return fields

class Slide:
    def __init__(self, index: int, title: str, text_contents: Optional[str], image_contents: Optional[str], poll: Poll, save_path: str, next_slide_name: str):
        self.index = index
        self.title = title
        self.text_contents = text_contents
        self.image_contents = image_contents
        self.poll = poll
        self.save_path = save_path
        self.next_slide_name = next_slide_name
    
    def export(self, slides_map):
        fields = {
            'index': self.index,
            'message': self.title,
            'slide': self.save_path,
            "emojis": [],
        }
        
        if self.poll is not None:
            serialized_poll = self.poll.to_json(slides_map)
            if serialized_poll is not None:
                fields.update(serialized_poll)
        else:
            # Base slide, no polls, resolve next_slide_name it's index in exported slide list
            next_slide = slides_map.get(self.next_slide_name)
            if self.next_slide_name is None or next_slide is None:
                print(f"Error can not resolve next slide for current slide {self.title}, {self.next_slide_name}")
                fields.update({
                    'next_slide_index': 0
                })
                return fields
            
            fields.update({
                'next_slide_index': next_slide.index
            })

        return fields


def slugify(title: str) -> str:
    title = re.sub(r'[^\w\s-]', '', title)
    title = re.sub(r'[\s_]+', '-', title)
    title = title.lower().strip('-')
    return title

def parse_contents_for_poll(text: str) -> Poll:
    """
    Parses a given text to extract the title and entries after "Contains Path Poll:"

    Returns:
        Maybe Poll
    """
    lines = text.strip().split('\n')
    title = None
    entries = []
    poll_type = None

    for line in lines:
        if "Contains Path Poll:" in line:
            poll_type = CONTAINS_PATH_POLL
            continue
        elif "State Poll" in line:
            poll_type = STATE_POLL
            continue

        if poll_type is not None:
            if line.startswith("*"):
                match = re.match(r'\* (.*?)\[\[(.*?)\]\]', line)
                if match:
                    entries.append((match.group(1), match.group(2)))
            elif title is None:
                title = line.strip()
    
    if poll_type == None:
        return None
    
        
    print(f"Parsed poll {title} {poll_type} {entries}")
    if title is None:
        print(f"Error Poll title can not be None!")

    return Poll(title, poll_type, entries)

def parse_md_file(file_path: str, index: int) -> Slide:
    """
    Parse a markdown file and extract slide and poll data.

    Parameters:
    file_path (str): The path to the markdown file.
    index (int): The index of the slide.

    Returns:
    Slide: An instance of the Slide class.
    """
    with open(file_path, 'r') as file:
        content = file.read()
    
    # Parse out possible poll
    maybe_poll = parse_contents_for_poll(content)

    # Parse out possible image contents in format ![[image_path.png]]
    slide_image_contents_match = re.search(r'\!\[\[(.*?)\]\]', content)
    slide_image_contents = slide_image_contents_match.group(1).strip() if slide_image_contents_match else None
    
    if slide_image_contents_match:
        print(f"Parsed out image: {slide_image_contents}")

    # Parse out possible text contents tagged with text
    slide_text_contents_match = re.search(r"^[Tt]ext: (.*)$", content, re.MULTILINE)
    slide_text_contents = slide_text_contents_match.group(1).strip() if slide_text_contents_match else content

    if slide_text_contents_match:
        print(f"Parsed out text: {slide_text_contents}")

    # Parse which slide comes next (if direct)
    next_slide_name_match = re.search(r'Next Slide:\s*\[\[(.*?)\]\]', content)
    next_slide_name = next_slide_name_match.group(1).strip() if next_slide_name_match else None

    # Parse filename to be set as slide message
    message = os.path.basename(file_path).split(".md")[0]
    message_slug = slugify(message)
    
    # Image path to save, referenced in slides.json as well
    save_path = f"{index:03d}.{message_slug}.png"

    return Slide(index, message, slide_text_contents, slide_image_contents, maybe_poll, save_path, next_slide_name)

def process_directory(md_dir: str, entry_slide_title: str) -> List[Slide]:
    """
    Process a directory of markdown files and generate slide data and images.

    Parameters:
    md_dir (str): The directory containing markdown files.
    output_dir (str): The directory to save the output images and slide json.

    Returns:
    List[Dict]: List of slide data as dictionaries.
    """
    slide_data = []
    md_files = [file for file in os.listdir(md_dir) if file.endswith('.md')]
    
    # Bump entry point file to top of list before applying index and parsing slide data
    index = next((i for i, f in enumerate(md_files) if f == entry_slide_title), None)
    if index is not None:
        md_files.insert(0, md_files.pop(index))

    for index, md_file in enumerate(md_files):
        file_path = os.path.join(md_dir, md_file)
        slide = parse_md_file(file_path, index)
        slide_data.append(slide)

    return slide_data


def generate_slide_image(slide: Slide, md_dir: str, output_dir: str):
    """
    Generate an image for the slide.

    Parameters:
    slide (Slide): An instance of the Slide class.
    output_dir (str): The directory to save the output images.
    """
    # TODO - this is just naive, draw centered text if slide has text, draw centered image if 
    # slide has image. Improve layout possibilities
    
    # TODO support emoji font 

    # First draw background image 
    base_img = Image.open(SLIDE_BACKGROUND_IMAGE_PATH).convert("RGBA")

    # Draw slide image if any
    if slide.image_contents is not None:
        slide_image_path = os.path.join(md_dir, slide.image_contents)
        if not os.path.exists(slide_image_path):
            print(f"Slide references image but image path could not be loaded! Slide{slide.title} image path {slide.image_contents}")
        
        print(f"Drawing slide image {slide_image_path} - {slide.image_contents}")
        slide_image = Image.open(slide_image_path).convert("RGBA")
        slide_image_width, slide_image_height = slide_image.size
        img_x = (1920 - slide_image_width) // 2
        img_y = (1080 - slide_image_height) // 2
        base_img.paste(slide_image, (img_x, img_y))

    # Draw text if any
    elif slide.text_contents is not None:
        draw = ImageDraw.Draw(base_img)
        font_size = 40
        font = ImageFont.truetype(FONT_PATH, font_size, encoding='unic')
        text_width, text_height = draw.textsize(slide.text_contents, font=font)
        text_x = (1920 - text_width) // 2
        text_y = (1080 - text_height) // 2
        
        text = slide.text_contents
        draw.text((text_x, text_y), text, fill=(105, 105, 105), font=font)  # Dark grey text

    output_path = os.path.join(output_dir, slide.save_path)
    base_img.save(output_path)


def main():
    parser = argparse.ArgumentParser(description="Process markdown files into slides data and generate images")
    parser.add_argument('md_dir', type=str, help="The directory containing markdown files")
    parser.add_argument('entry_md', type=str, help="The starting slide")
    parser.add_argument('output_dir', type=str, help="The directory to save the output images")
    parser.add_argument('gen_images', type=str, help="True to skip image render output generation")
    args = parser.parse_args()

    md_dir = args.md_dir
    output_dir = args.output_dir
    entry_slide_title = os.path.basename(args.entry_md)

    if not os.path.exists(output_dir):
        os.makedirs(output_dir)

    # Initial markdown parsing to create Slide and Poll class objects
    slides = process_directory(md_dir, entry_slide_title)

    # Create slide lookup keyed on slide name for final slide data 'advance_to_slide' json processing
    slide_map = {}
    for slide in slides:
        slide_map[slide.title] = slide
        
    slides_json = []
    if args.gen_images:
        print(f"Generation {len(slides_json)} images...")
    for slide in slides: 
        # Export slide image
        if args.gen_images:
            generate_slide_image(slide, md_dir, output_dir)
        # Serialize and resolve all polls and linked slides by name to index with slide_map
        slides_json.append(slide.export(slide_map))

    # Save slide data to a JSON file
    with open(os.path.join(output_dir, 'slides.json'), 'w') as json_file:
        json.dump(slides_json, json_file, indent=4, ensure_ascii=False)

    print(f"Exported {len(slides_json)} slides")

if __name__ == "__main__":
    main()




