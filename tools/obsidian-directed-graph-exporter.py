"""
Usage

python -m venv venv
source venv/bin/activate
pip install -r requirements.txt
python obsidian-directed-graph-exporter.py {{path_to_markdown_files}} {{entrypoint md}} {{path_to_output_images}} {{gen_images true|false}}
"""

import os
import re
import json
import argparse
import sys
from typing import List, Optional, Tuple
from PIL import Image, ImageDraw, ImageFont
from pygments import highlight
from pygments.lexers import RustLexer
from pygments.formatters import ImageFormatter

FONT_PATH = "./assets/fonts/Open_Sans/OpenSans-Italic-VariableFont_wdth,wght.ttf"
# FONT_PATH = "./assets/fonts/AppleColorEmoji.ttf"
SLIDE_BACKGROUND_IMAGE_PATH = "./assets/background_lightmode.png"
CONTAINS_PATH_POLL = "CONTAINS_PATH_POLL"
STATE_POLL = "STATE_POLL"

# Frontend will break if poll names are not unique, detect and throw error
seen_poll_names = set()


class PollRenderConfig:
    def __init__(self, x: int, y: int, scale: int, vbx: int, vby: int):
        self.x = x
        self.y = y
        self.scale = scale
        self.vbx = vbx
        self.vby = vby


class Poll:
    def __init__(self, name: str, poll_type: str, options: List[Tuple[str, str]], pollRenderConfig: PollRenderConfig = None): 
        self.name = name
        self.poll_type = poll_type
        self.options = options   # List (Choice text, Destination slide title)
        self.pollRenderConfig = pollRenderConfig

        if poll_type == STATE_POLL:
            return
        if name in seen_poll_names:
            print(f"Unique poll name violation, {name} seen twice")
            sys.exit(1)
        seen_poll_names.add(name)
        
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
            pollRender = self.pollRenderConfig if self.pollRenderConfig is not None else PollRenderConfig(68, 2, 30, 230, 230)
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
                    'refresh_interval': 1,
                    'type_': 'centroid',
                    'x': pollRender.x,
                    'y': pollRender.y,
                    'scale': pollRender.scale,
                    'vbx': pollRender.vbx,
                    'vby': pollRender.vby
                }
            })
        return fields


class SlideImage:
    def __init__(self, path: str, scale: float):
        self.path = path
        self.scale = scale


class Slide:
    def __init__(self, index: int, title: str, title_override: str, text_contents: Optional[str], image: Optional[SlideImage], poll: Poll, save_path: str, next_slide_name: str, emojis: List[str], fontScale: float):
        self.index = index
        self.title = title
        self.title_override = title_override
        self.text_contents = text_contents
        self.image = image
        self.poll = poll
        self.save_path = save_path
        self.next_slide_name = next_slide_name
        self.emojis = emojis
        self.fontScale = fontScale
    
    def export(self, slides_map):
        fields = {
            'index': self.index,
            'message': self.title if self.title_override is not None else self.title_override,
            'slide': self.save_path,
            "emojis": self.emojis if len(self.emojis) else ["🇨🇦", "😃"] ,
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
                print(f"{slides_map}")
                sys.exit(1)
            
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

def parse_contents_for_poll_render_metadata(content: str) -> PollRenderConfig:
    try:
        match = re.search(r"^Poll Render X: (.*)$", content, re.MULTILINE)
        x = int(match.group(1).strip()) 
        match = re.search(r"^Poll Render Y: (.*)$", content, re.MULTILINE)
        y = int(match.group(1).strip()) 
        match = re.search(r"^Poll Render Scale: (.*)$", content, re.MULTILINE)
        scale = int(match.group(1).strip()) 
        match = re.search(r"^Poll Render VBX: (.*)$", content, re.MULTILINE)
        vbx = int(match.group(1).strip()) 
        match = re.search(r"^Poll Render VBY: (.*)$", content, re.MULTILINE)
        vby = int(match.group(1).strip()) 
        return PollRenderConfig(x, y, scale, vbx, vby)
    except Exception as e:
        return None

def parse_md_file(md_dir: str, file_path: str, index: int) -> Slide:
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
    
    # Parse out possible emojis
    emojis_match = re.search(r'^Emojis:\s*(.*)$', content, re.MULTILINE)
    emojis = emojis_match.group(1).split(",") if emojis_match else []
    emojis = [e.strip() for e in emojis]

    # Parse out possible poll
    maybe_poll = parse_contents_for_poll(content)
    # Parse out possible poll render options
    maybe_poll_render_options = parse_contents_for_poll_render_metadata(content)
    if maybe_poll is not None:
        print(f"applying parsed poll render options: {maybe_poll_render_options}")
        maybe_poll.pollRenderConfig = maybe_poll_render_options

    # Parse which slide comes next (if direct)
    next_slide_name_match = re.search(r'Next Slide:\s*\[\[(.*?)\]\]', content)
    next_slide_name = next_slide_name_match.group(1).strip() if next_slide_name_match else None

    # Parse filename to be set as slide message
    message = os.path.basename(file_path).split(".md")[0]
    message_slug = slugify(message)

    # Parse out possible image contents in format ![[image_path.png]]
    match = re.search(r'\!\[\[(.*?)\]\]', content)
    slide_image_name = match.group(1).strip() if match else None
    slide_image = None
    if match:
        slide_image_scale_match = re.search(r'Scale:\s*(.*)', content)
        scale = slide_image_scale_match.group(1).strip() if slide_image_scale_match else 1
        slide_image = SlideImage(os.path.join(md_dir, slide_image_name), float(scale))
        print(f"Parsed out image: {slide_image.path} with scale {slide_image.scale}")

    # Parse out possible text contents tagged with text
    match = re.search(r"^[Tt]ext: (.*)$", content, re.MULTILINE)
    slide_text_contents = match.group(1).strip() if match else content
    if match:
        print(f"Parsed out text: {slide_text_contents}")
    
    # Parse out possible `Font Scale:` metadata tag
    match = re.search(r"^Font Scale: (.*)$", content, re.MULTILINE)
    text_contents_font_scale = float(match.group(1).strip()) if match else 1
    
    # Parse out possible `Title Override:` metadata tag
    match = re.search(r"^Title Override: (.*)$", content, re.MULTILINE)
    message_override = match.group(1).strip() if match else message

    # Parse out if the contents of the slide contain Rust code
    match = re.search(r"```rust((.|\n)*)```", content, re.MULTILINE)
    slide_code_contents = match.group(1).strip() if match else content
    if match:
        print(f"Parsed out code: {slide_code_contents}") 
        lexer = RustLexer()
        formatter = ImageFormatter(font_name='Andale Mono', line_numbers=False, style='monokai', font_size=42)
        slide_image = SlideImage(f"output_code_images/{index:03d}.{message_slug}_code.png", 1)
        
        # Highlight the code and save it to an image
        with open(slide_image.path, "wb") as f:
            highlight(slide_code_contents, lexer, formatter, outfile=f)
        
        print(f"Wrote out code image: {slide_image.path}")
    
    # Image path to save, referenced in slides.json as well
    save_path = f"{index:03d}.{message_slug}.png"

    return Slide(index, message, message_override, slide_text_contents, slide_image, maybe_poll, save_path, next_slide_name, emojis, text_contents_font_scale)

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
        slide = parse_md_file(md_dir, file_path, index)
        slide_data.append(slide)

    return slide_data


def generate_slide_image(slide: Slide, md_dir: str, output_dir: str):
    """
    Generate an image for the slide.

    Parameters:
    slide (Slide): An instance of the Slide class.
    output_dir (str): The directory to save the output images.
    """
    
    # First draw background image 
    base_img = Image.open(SLIDE_BACKGROUND_IMAGE_PATH).convert("RGBA")
    
    # Store the dimensions of the base image so we know how to scale stuff in the future
    base_img_x = base_img.size[0]
    base_img_y = base_img.size[1]

    # Draw slide image if any
    if slide.image is not None:
        print(f"Drawing slide {slide.title} With Image - {slide.image.path}")
        if not os.path.exists(slide.image.path):
            print(f"Slide references image but image path could not be loaded! Slide {slide.title} image path {slide.image.path}")
        
        slide_image = Image.open(slide.image.path).convert("RGBA")

        x_scale = slide_image.size[0] / base_img_x
        y_scale = slide_image.size[1] / base_img_y

        # Scale image to fit within base image
        if x_scale > 1 or y_scale > 1:
            scale = max(x_scale, y_scale)
            slide_image = slide_image.resize((int(slide_image.size[0] / scale), int(slide_image.size[1] / scale)))
        else:
            slide_image = slide_image.resize((int(slide_image.size[0] * slide.image.scale), int(slide_image.size[1] * slide.image.scale)))

        slide_image_width, slide_image_height = slide_image.size
        img_x = (1920 - slide_image_width) // 2
        img_y = (1080 - slide_image_height) // 2
        base_img.paste(slide_image, (img_x, img_y), slide_image)

    # Draw text if any
    elif slide.text_contents is not None:
        print(f"Drawing slide {slide.title} With Text - {slide.text_contents}")
        text = slide.text_contents

        scale = slide.fontScale
        if slide.fontScale == 1 and len(text) > 30:
            scale = 0.3
        font_size = int(160 * slide.fontScale)
        if not text.isascii:
            font_size = 160 # font size must be 160 if emojis and text in string

        font = ImageFont.truetype("./assets/fonts/Export/SFProDisplay/OpenType-TT/SF-Pro-Display-Medium.ttf", font_size)
        emojiFont = ImageFont.truetype("/System/Library/Fonts/Apple Color Emoji.ttc", 160)

        draw = ImageDraw.Draw(base_img)
        text_width, text_height = draw.textsize(text, font=font)
        text_x = (1920 - text_width) // 2
        text_y = (1080 - text_height) // 2
        
        draw.text((text_x, text_y), text, fill=(105, 105, 105), font=font)  # Dark grey text

        addEmojis(base_img,  text, (text_x, text_y), font, emojiFont, 165)

    output_path = os.path.join(output_dir, slide.save_path)
    base_img.save(output_path)


def getEmojiMask(font: ImageFont, emoji: str, size: tuple[int, int]) -> Image:
    """ Makes an image with an emoji using AppleColorEmoji.ttf, this can then be pasted onto the image to show emojis
    
    Parameter:
    (ImageFont)font: The font with the emojis (AppleColorEmoji.ttf); Passed in so font is only loaded once
    (str)emoji: The unicoded emoji
    (tuple[int, int])size: The size of the mask
    
    Returns:
    (Image): A transparent image with the emoji
    
    """

    mask = Image.new("RGBA", (160, 160), color=(255, 255, 255, 0))
    draw = ImageDraw.Draw(mask)
    draw.text((0, 0), emoji, font=font, embedded_color=True)
    mask = mask.resize(size)

    return mask

def getDimensions(draw: ImageDraw, text: str, font: ImageFont) -> tuple[int, int]:
    """ Gets the size of text using the font
    
    Parameters:
    (ImageDraw): The draw object of the image
    (str)text: The text you are getting the size of
    (ImageFont)font: The font being used in drawing the text
    
    Returns:
    (tuple[int, int]): The width and height of the text
    
    """
    left, top, right, bottom = draw.multiline_textbbox((0, 0), text, font=font)
    return (right-left), (bottom-top)

def addEmojis():
    # Now add any emojis that weren't embedded correctly
    modifiedResponseL = modifiedResponse.split("\n")
    for i, line in enumerate(modifiedResponseL):
        for j, char in enumerate(line):
            if (not char.isascii()):
                
                # Get the height of the text ABOVE the emoji in modifiedResponse
                aboveText = "\n".join(modifiedResponseL[:i])
                _, aboveTextHeight = getDimensions(draw, aboveText, poppinsFont)

                # The height that we paste at is aboveTextHeight + (marginHeight+PADDING) + (Some error)
                # (marginHeight+PADDING) is where we pasted the entire paragraph
                y = aboveTextHeight + (marginHeight+PADDING) + 5

                # Get the length of the text on the line up to the emoji
                beforeLength, _ = getDimensions(draw, line[:j], poppinsFont)

                # The x position is beforeLength + 75; 75px is where we pasted the entire paragraph
                x = (75) + beforeLength

                # Create the mask
                emojiMask = getEmojiMask(emojiFont, char, (165, 165))

                # Paste the mask onto the image
                img.paste(emojiMask, (int(x), int(y)), emojiMask)

def addEmojis(img: Image, text: str, box: tuple[int, int], font: ImageFont, emojiFont: ImageFont, size: int) -> None:
    """ Adds emojis to the text
    
    Parameters:
    (Image)img: The image to paste the emojis onto
    (tuple[int, int])box: The (x,y) pair where the textbox is placed
    (ImageFont)font: The font of the text
    (ImageFont)emojiFont: The emoji's font
    
    """
    draw = ImageDraw.Draw(img)
    width, height = box
    # Now add any emojis that weren't embedded correctly
    text_lines = text.split("\n")
    for i, line in enumerate(text_lines):
        for j, char in enumerate(line):
            if (not char.isascii()):
                
                # Get the height of the text ABOVE the emoji in modifiedResponse
                aboveText = "\n".join(text_lines[:i])
                _, aboveTextHeight = getDimensions(draw, aboveText, font)

                # The height that we paste at is aboveTextHeight + height + (Some error)
                y = aboveTextHeight + height + 5

                # Get the length of the text on the line up to the emoji
                beforeLength, _ = getDimensions(draw, line[:j], font)

                # The x position is beforeLength + width
                x = width + beforeLength

                # Create the mask; You might want to adjust the size parameter
                emojiMask = getEmojiMask(emojiFont, char, (size, size))

                # Paste the mask onto the image
                img.paste(emojiMask, (int(x), int(y)), emojiMask)


def main():
    # for i in range(0, 1000):
    #     try:
    #         font = ImageFont.truetype("/System/Library/Fonts/Apple Color Emoji.ttc", size=i)
    #         print(f"Font size {i} loaded")
    #     except:
    #         pass
    
    # exit(0)

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




