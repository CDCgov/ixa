from cfa.cloudops import CloudClient

def main():
    # initialize
    print("Initializing CloudClient...")
    cc = CloudClient(dotenv_path=".env", use_sp = True)
    print("Hello from ixa-cfa-cloudops!")
    files = cc.list_blob_files("input-test")
    print(f"Files in 'input-test' container: {files}")

if __name__ == "__main__":
    main()
