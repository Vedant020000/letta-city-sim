type Props = {
  repoSlug: string;
};

export function TownhallFooter({ repoSlug }: Props) {
  return (
    <footer className="footer">
      built for the letta community - <a href={`https://github.com/${repoSlug}`}>{repoSlug}</a>
    </footer>
  );
}
